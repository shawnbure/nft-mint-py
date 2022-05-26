elrond_wasm::imports!();
elrond_wasm::derive_imports!();

use elrond_wasm::elrond_codec::TopEncode;

const NFT_AMOUNT: u32 = 1;
const ROYALTIES_MAX: u32 = 10_000;

#[derive(TypeAbi, TopEncode, TopDecode)]
pub struct PriceTag<M: ManagedTypeApi> {
    pub token: TokenIdentifier<M>,
    pub nonce: u64,
    pub amount: BigUint<M>,
}

#[elrond_wasm::module]
pub trait NftModule {
    // endpoints - owner-only

    #[only_owner]
    #[payable("EGLD")]
    #[endpoint(issueToken)]
    fn issue_token(&self, 
                   token_name: ManagedBuffer,       // (KarmaStickma: 4B61726D61537469636B6D61) hexed - between 3-20 alphanumeric characters only (https://www.rapidtables.com/convert/number/ascii-to-hex.html) 
                   token_ticker: ManagedBuffer)     // (KARMASTICK: 4B41524D41535449434B) hexed  - Between 3-10 UpperCase Character
    {
        require!(self.nft_token_id().is_empty(), "Token already issued");

        let payment_amount = self.call_value().egld_value();
        self.send()
            .esdt_system_sc_proxy()
            .issue_non_fungible(
                payment_amount,
                &token_name,
                &token_ticker,
                NonFungibleTokenProperties {
                    can_freeze: true,
                    can_wipe: true,
                    can_pause: true,
                    can_change_owner: false,
                    can_upgrade: false,
                    can_add_special_roles: true,
                },
            )
            .async_call()
            .with_callback(self.callbacks().issue_callback())
            .call_and_exit()
    }

    #[only_owner]
    #[endpoint(setLocalRoles)]
    fn set_local_roles(&self) {
        self.require_token_issued();

        self.send()
            .esdt_system_sc_proxy()
            .set_special_roles(
                &self.blockchain().get_sc_address(),
                &self.nft_token_id().get(),
                [EsdtLocalRole::NftCreate][..].iter().cloned(),
            )
            .async_call()
            .call_and_exit()
    }

    // endpoints


    //https://docs.elrond.com/wallet/webhooks/
    //https://devnet-wallet.elrond.com/send

    //https://devnet-wallet.elrond.com/hook/transaction?receiver=erd1hh7gte28hahk9htwlhzf3gretusckhrqf4y6xv5p9qwznhn7y4wswnzua3&value=1&gasLimit=250000000&data=buyNft&callbackUrl=https://www.msn.com/
    
    
    #[payable("*")]
    #[endpoint(buyNft)]
    fn buy_nft(&self, nft_nonce: u64)  -> SCResult<()> 
    {
        let payment: EsdtTokenPayment<Self::Api> = self.call_value().payment();

        self.require_token_issued();

        if self.price_tag(nft_nonce).is_empty()
        {
            return sc_error!("Invalid nonce or NFT was already sold")
        }


        let price_tag = self.price_tag(nft_nonce).get();

        if payment.token_identifier != price_tag.token
        {
            return sc_error!("Invalid token used as payment")
        }

        if payment.token_nonce != price_tag.nonce
        {
            return sc_error!("Invalid nonce for payment token")
        }
                

        if payment.amount != price_tag.amount
        {
            return sc_error!("Invalid amount as payment")
        }

        self.price_tag(nft_nonce).clear();

        let nft_token_id = self.nft_token_id().get();
        let caller = self.blockchain().get_caller();

        self.send().sell_nft( &nft_token_id,
                                nft_nonce,
                                &BigUint::from(NFT_AMOUNT),
                                &caller,
                                &payment.token_identifier,
                                payment.token_nonce,
                                &payment.amount);
                   
            
        Ok(())
    }




    #[payable("EGLD")]
    #[only_owner]
    #[endpoint(sendNFT)]
    fn send_nft(&self, 
                nonce: u64,
                receiver_address: ManagedAddress) -> SCResult<()>
    {
        self.require_token_issued();

        if self.price_tag(nonce).is_empty()
        {
            return sc_error!("Invalid nonce or NFT was already sold")
        }
       
        let nft_token_id = self.nft_token_id().get();

        self.send().direct(
            &receiver_address,          
            &nft_token_id,
            nonce,
            &BigUint::from(NFT_AMOUNT),
            &[],
        );

        Ok(())
    }
    
    



    //get contract balance
    #[view(getContractEGLDBalance)]
    fn get_contract_egld_balance(&self) -> BigUint 
    {  
       return self.blockchain().get_balance(&self.blockchain().get_sc_address());
    }

    
   //get contract balance
   #[payable("EGLD")]
   #[only_owner]
   #[endpoint(withdrawContractEGLDBalance)]
   fn withdraw_contract_egld_balance(&self) -> SCResult<()>  
   {    
        let amount_to_withdraw = self.blockchain().get_balance(&self.blockchain().get_sc_address());

        let receiver_address = self.blockchain().get_caller();
        
        self.send().direct_egld(&receiver_address, &amount_to_withdraw, &[]);
        Ok(())
   }




    // views

    #[allow(clippy::type_complexity)]
    #[view(getNftPrice)]
    fn get_nft_price(
        &self,
        nft_nonce: u64,
    ) -> OptionalValue<MultiValue3<TokenIdentifier, u64, BigUint>> {
        if self.price_tag(nft_nonce).is_empty() {
            // NFT was already sold
            OptionalValue::None
        } else {
            let price_tag = self.price_tag(nft_nonce).get();

            OptionalValue::Some((price_tag.token, price_tag.nonce, price_tag.amount).into())
        }
    }

    // callbacks

    #[callback]
    fn issue_callback(&self, #[call_result] result: ManagedAsyncCallResult<TokenIdentifier>) {
        match result {
            ManagedAsyncCallResult::Ok(token_id) => {
                self.nft_token_id().set(&token_id);
            },
            ManagedAsyncCallResult::Err(_) => {
                let caller = self.blockchain().get_owner_address();
                let (returned_tokens, token_id) = self.call_value().payment_token_pair();
                if token_id.is_egld() && returned_tokens > 0 {
                    self.send()
                        .direct(&caller, &token_id, 0, &returned_tokens, &[]);
                }
            },
        }
    }

    // private

    #[allow(clippy::too_many_arguments)]
    fn create_nft_with_attributes<T: TopEncode>(
        &self,
        name: ManagedBuffer,
        royalties: BigUint,
        attributes: T,
        uri: ManagedBuffer,
        uri_json: ManagedBuffer,
        selling_price: BigUint,
        token_used_as_payment: TokenIdentifier,
        token_used_as_payment_nonce: u64,
    ) -> u64 {
        self.require_token_issued();
        require!(royalties <= ROYALTIES_MAX, "Royalties cannot exceed 100%");

        let nft_token_id = self.nft_token_id().get();

        let mut serialized_attributes = ManagedBuffer::new();
        if let core::result::Result::Err(err) = attributes.top_encode(&mut serialized_attributes) {
            sc_panic!("Attributes encode error: {}", err.message_bytes());
        }

        let attributes_sha256 = self.crypto().sha256(&serialized_attributes);
        let attributes_hash = attributes_sha256.as_managed_buffer();

        let mut uris = ManagedVec::new();
        uris.push(uri);
        uris.push(uri_json);
        
        let nft_nonce = self.send().esdt_nft_create(
            &nft_token_id,
            &BigUint::from(NFT_AMOUNT),
            &name,
            &royalties,
            attributes_hash,
            &attributes,
            &uris,
        );

        self.price_tag(nft_nonce).set(&PriceTag {
            token: token_used_as_payment,
            nonce: token_used_as_payment_nonce,
            amount: selling_price,
        });

        nft_nonce
    }

    fn require_token_issued(&self) {
        require!(!self.nft_token_id().is_empty(), "Token not issued");
    }

    // storage

    #[storage_mapper("nftTokenId")]
    fn nft_token_id(&self) -> SingleValueMapper<TokenIdentifier>;

    #[storage_mapper("priceTag")]
    fn price_tag(&self, nft_nonce: u64) -> SingleValueMapper<PriceTag<Self::Api>>;

    
    #[view(getVersion)]
    #[storage_mapper("version")]
    fn version(&self) -> SingleValueMapper<ManagedBuffer>;    
}
