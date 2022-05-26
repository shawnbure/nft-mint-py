from time import sleep
import erdpy, logging, json, os, binascii, subprocess, base64, configparser

from argparse import ArgumentParser
from erdpy import utils, config
from erdpy.accounts import Account, Address
from erdpy.proxy import ElrondProxy
from erdpy.transactions import BunchOfTransactions
import requests


logging.basicConfig(level=logging.DEBUG, filename='app.log', filemode='w', format='%(name)s - %(levelname)s - %(message)s')


#############################################   INIT   #############################################

#made config file for ease of use
config = configparser.RawConfigParser()
config.read('config.cfg')

global_config_dictionary = dict(config.items('GLOBAL'))
deploy_config_dictionary = ""

#This will create a production version or devnet version - #False = development, True = production
if int(global_config_dictionary['deploy_to_production']) == 1:
    deploy_config_dictionary = dict(config.items('PRODUCTION'))
else:
    deploy_config_dictionary = dict(config.items('DEVELOPMENT'))


ONE_EGLD_STR = "1" + "0" * 18
ONE_EGLD = int(ONE_EGLD_STR)
GAS_PRICE = 1000000000

#transaction request sze is limited because of the HTTP post size
TRANSACTION_REQUEST_SIZE = 50






#############################################   MAIN   #############################################
def main():
    
    print("========== EXECUTING bulk.py ==========")
    bulk_mint()  






#############################################   BULK MINTER   #############################################

def bulk_mint():

    # proxy address for elrond
    contract_address = deploy_config_dictionary['contract_address']
    proxy_address = deploy_config_dictionary['proxy_address']
    chain_id = deploy_config_dictionary['chain'] 

    #base cid for pics at nft.storage
    base_cid = deploy_config_dictionary['base_cid']

    #base uri for pics at nft.storage
    base_uri = deploy_config_dictionary['base_uri']

    # needs to be the hex version of the number 1000 which is 10% [and not the hex vesion os ASCII 1000]
    royalties = deploy_config_dictionary['royalties']

    # needs to be the hex version of the number 050000000000000000 which is .5 egld [and not the hex vesion os ASCII 050000000000000000]
    selling_price = deploy_config_dictionary['selling_price']

    # json metadata file
    json_path = deploy_config_dictionary['json_path']

    #get the file object 
    json_metadata_file = open(json_path,mode='r')

    #read all of the lines
    json_metadata = json_metadata_file.read()

    #close the file
    json_metadata_file.close()

    # parse json metadata:
    metadata_dictionary = json.loads(json_metadata)

    #payload counter
    dictionary_count = len(metadata_dictionary)

    #build the payload

    https_url = "https://" +  base_uri + base_cid + "/"

    #transaction counters
    cost = 0
    transaction_counter = 0
    transaction_batch_counter = 0
  
    # get the transaction_bunch
    bunch = transaction_bunch()

    while transaction_counter < dictionary_count:

        nft_name = metadata_dictionary[transaction_counter]["name"].encode('utf-8').hex()

        nft_uri = (https_url + metadata_dictionary[transaction_counter]["image"]).encode('utf-8').hex()
        nft_uri_json = (https_url + metadata_dictionary[transaction_counter]["metadata"]).encode('utf-8').hex()


        #metadata:ipfs.io/ipfs/bafybeig4lkqxri3zi7ocbgthwjxjikcdy6hv3isjfg77fdl7jds5llk6h4/50.json;tags:karmasutra,kamasutra,NSFW,sexy,adult,Top Reverse,Male - Male,Karma-Stickma,animated,gif,NFT

        #https://ipfs.io/ipfs/bafybeig4lkqxri3zi7ocbgthwjxjikcdy6hv3isjfg77fdl7jds5llk6h4/KarmaStickma_Doggy_MM_s6_342.gif
        #metadata:ipfscid/metadata.json;tags:tag1,tag2,..,tagN
        attributes_raw = "metadata:" + (base_cid + "/" + metadata_dictionary[transaction_counter]["metadata"]) + ";"

        #metadata:QmXkoStCA4xzpTxbZRR8Ex1G4GCGoStxMSWHZistHTj715/4436.json;tags:Drifters,Settlers,4436

        #get attributes dictionary into a string
        attributes_raw += "tags:" + metadata_dictionary[transaction_counter]["tags"]
        nft_attributes = attributes_raw.encode('utf-8').hex()



        nft_royalties = f"{int(royalties):X}".zfill(16)
        nft_selling_price = f"{int(selling_price):X}".zfill(16)

        #nft name, nft uri, image, attributes, hash, royalties, selling price, token, payment nonce - @ with no value is null
        data = "createNft@" + nft_name + "@" + nft_uri + "@" + nft_uri_json + "@" + nft_royalties + "@" + nft_selling_price + "@" + nft_attributes    #proxy_address = https://devnet-gateway.elrond.com
    

        logging.debug(metadata_dictionary[transaction_counter]["name"])

        #print("DATA: " + data)
        bunch.add(data)
        transaction_batch_counter += 1
        transaction_counter += 1

     
        #limit the transaction batch size
        if transaction_batch_counter == TRANSACTION_REQUEST_SIZE:
            print("==== Batch Count Reached - Sending Bulk Tx")  
            transaction_batch_counter = 0

            if send_bulk_tx(bunch):                
                bunch = transaction_bunch()                 
                print("Sent", transaction_counter, " mint transaction(s).")  
                            
            else:
                bunch = transaction_bunch()
                transaction_counter -= TRANSACTION_REQUEST_SIZE 
                print("Redo Bunch")


    if transaction_batch_counter > 0:
        #send the remaining transactions
        print("==== Remaining Batch - Sending Bulk Tx") 
        send_bulk_tx(bunch)
        print("Sent", transaction_counter, "mint transaction(s).")







def send_bulk_tx(bunch):

    apiUrl = deploy_config_dictionary['api_url']
    _,hashes = bunch.send()

    sleep(3)

    if len(hashes) == 0 :
        print("Hash Empty")
        sleep(2)
        return False
            
    isPending = True

    countTracker = 1

    while isPending == True: 
        hashComma = ''
        for key in hashes:
            hashComma=hashComma+hashes[key]+','
        res = requests.get(apiUrl+hashComma[:-1])
        results=res.json()
        
        
        for r in results:
            print("Status [" + str(countTracker) + "]: " + r["status"])
            countTracker += 1

            if r["status"]=="fail" :           
                return False
            elif r["status"]=="pending" :
                isPending = True
                sleep(2)
                continue
            else:
                return True    
    




class transaction_bunch:

    # FYI - The HTTP POST can not be larger then standard limits. Send 100 per request for now
        
    #contract address to deploy to
    contract_address = deploy_config_dictionary['contract_address']

    # wallet file for deployment
    pem = deploy_config_dictionary['pem']

    # proxy address for elrond
    proxy_address = deploy_config_dictionary['proxy_address']

    # chain id for elrond 
    chain_id = deploy_config_dictionary['chain'] 

    #payload counters
    transaction_counter = 0
    transaction_batch_counter = 0
    tx_version = 1
    options = ""    
    value = "0"

    # The init method or constructor
    def __init__(self):
        self.parser = ArgumentParser()
        self.parser.add_argument("--proxy", default=self.proxy_address)
        self.parser.add_argument("--pem", default=self.pem)
        self.args = self.parser.parse_args()
        self.proxy = ElrondProxy(self.args.proxy)
        self.sender = Account(pem_file=self.args.pem)
        self.sender.sync_nonce(self.proxy)
        self.bunch = BunchOfTransactions()

    # adds an item to the bunch
    def add(self, data): 

		#seems to be a moving target and mystery. I used 20MM for a while
        gas_limit = 20000000

        #bunch.add(sender, address.bech32(), sender.nonce, str(value), data, GAS_PRICE, gas_limit, chain_id, tx_version, options)
        self.bunch.add(self.sender, Address(self.contract_address).bech32(), self.sender.nonce, self.value, data, GAS_PRICE, gas_limit, self.chain_id, self.tx_version, self.options)
        self.sender.nonce += 1

    # send the bunch    
    def send(self):    
        return self.bunch.send(self.proxy)   



if __name__ == "__main__":
    main()