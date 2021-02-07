use crate::net::http::{JsonRpcParam, RequestFactory};
use crate::net::http::JsonRpc;
use crate::settings::BtcNodeSettings;
use crate::bitcoin::bitcoin_core_responses::NetworkInfo;
use crate::bitcoin::bitcoin_core_responses::NetworkInfoResult;
use crate::bitcoin::bitcoin_core_responses::ListUnspentResponse;
use crate::bitcoin::bitcoin_core_responses::SendRawTxResponse;
use bitcoin::util::address::Address;
use bitcoin::util::psbt::serialize::Serialize;
use bitcoin::Transaction;

use super::bitcoin_core_responses::{BlockCountResponse, JsonRpcResponse};

pub struct BitcoinCore {
    settings : BtcNodeSettings,
    req_factory : RequestFactory
}

pub enum BTC_CORE_RPC_TYPES {
    GET_NETWORK_INFO,
    LIST_UNSPENT,
    SEND_RAW_TRANSACTION,
    GET_BLOCK_COUNT
}

impl BitcoinCore {
    /// Creates a new object of the BitcoinCore struct which can
    /// be used to send RPC request to a BitcoinCore node
    /// 
    /// # Arguments
    /// 
    /// * `settings` Connection settings (url, port, authentication)
    pub fn new(settings : BtcNodeSettings, req_factory : RequestFactory) -> BitcoinCore {
        BitcoinCore {
            settings,
            req_factory
        }
    }

    /// Query basic info about the node
    pub fn get_network_info(&self) -> Result<NetworkInfoResult,String> {
        let rpc = JsonRpc::new(String::from("1.0"), self.settings.id.clone(), String::from("getnetworkinfo"), vec![]);
        let url = format!("http://{}:{}", self.settings.url, self.settings.port);
        let req = self.req_factory.new_json_rpc_request(url, rpc, self.settings.user.clone(), self.settings.pass.clone());
        match req.execute() {
            Ok(x) => {
                println!("{}", x.content);
                let parsed : NetworkInfo = serde_json::from_str(&x.content)
                    .expect("Failed to parse networkinfo Json response");
                if parsed.id != self.settings.id {
                    Err("RPC Request and Response id didn't match!".to_string())
                }
                else {
                    if parsed.error.is_some() {
                        Err(parsed.error.unwrap().message)
                    }
                    else {
                        Ok(parsed.result.expect("GetNetworkInfo result was empty"))
                    }
                }
            },
            Err(e) => Err(e.to_string())
        }
    }

    /// Import a new address into the node to be able to check its balance
    /// Note that we call the RPC method with rescan = false that means it will
    /// only pick up balances coming from transactions in future blocks
    /// 
    /// # Arguments
    /// 
    /// * `addr` the address to index
    pub fn import_btc_address(&self, addr : Address) -> Result<(), String> {
        let mut params : Vec<JsonRpcParam> = Vec::new();
        params.push(JsonRpcParam::String(addr.to_string()));
        params.push(JsonRpcParam::String(String::from("")));
        params.push(JsonRpcParam::Bool(false));
        let rpc = JsonRpc::new(String::from("1.0"), self.settings.id.clone(), String::from("importaddress"), params);
        let url = format!("http://{}:{}", self.settings.url, self.settings.port);
        let req = self.req_factory.new_json_rpc_request(url, rpc, self.settings.user.clone(), self.settings.pass.clone());
        match req.execute() {
            Ok(x) => {
                let parsed : JsonRpcResponse<()> = serde_json::from_str(&x.content)
                    .expect("Failed to parse import_btc_address response");
                if parsed.id != self.settings.id {
                    Err("RPC Request and Response id didn't match".to_string())
                }
                else {
                    if parsed.error.is_some() {
                        Err(parsed.error.unwrap().message)
                    }
                    else {
                        Ok(())
                    }
                }
            }
            Err(e) => {
                Err(e.to_string())
            }
        }
    }

    /// Get the final unspent balance of an address represented as a string
    /// 
    /// # Arguments
    /// 
    /// * `addr` the address for which to check the balance
    pub fn get_address_final_balance(&self, addr: String) -> Result<u64, String> {
        let mut params : Vec<JsonRpcParam> = Vec::new();
        params.push(JsonRpcParam::Int(1));
        params.push(JsonRpcParam::Int(9999999));
        params.push(JsonRpcParam::Vec(vec![addr]));
        let rpc = JsonRpc::new(String::from("1.0"), self.settings.id.clone(), String::from("listunspent"), params);
        let url = self.get_url();
        let req = self.req_factory.new_json_rpc_request(url, rpc, self.settings.user.clone(), self.settings.pass.clone());
        match req.execute() {
            Ok(x) => {
                println!("{}", x.content);
                let parsed : ListUnspentResponse = serde_json::from_str(&x.content)
                    .expect("Failed to parse listunspent rpc response");
                if parsed.id != self.settings.id {
                    Err("RPC Request and Response id didn't match!".to_string())
                }
                else {
                    if parsed.error.is_some() {
                        Err(parsed.error.unwrap().message)
                    }
                    else {
                        let mut balance : u64 = 0;
                        // Sum up the unspent balances of the UTXOs under this address
                        for e in &parsed.result.expect("GetAddressFinalBalance Result was empty") {
                            let sat_amount = (e.amount * 100_000_000.0) as u64;
                            balance = balance + sat_amount;
                        }
                        Ok(balance)
                    }
                }
            }
            Err(e) => Err(e.to_string())
        }
    }

    /// Query the current blockheight of the bitcoin blockchain
    pub fn get_current_block_height(&self) -> Result<u64, String> {
        
        let rpc = JsonRpc::new(String::from("1.0"), self.settings.id.clone(), String::from("getblockcount"), vec![]);
        let url = self.get_url();
        let req = self.req_factory.new_json_rpc_request(url, rpc, self.settings.user.clone(), self.settings.pass.clone());
        match req.execute() {
            Ok(x) => {
                println!("{}", x.content);
                let parsed : BlockCountResponse = serde_json::from_str(&x.content)
                    .unwrap();
                if parsed.id != self.settings.id {
                    Err(String::from("RPC Request and Response id mismacht"))
                }
                else {
                    if parsed.error.is_some() {
                        Err(parsed.error.unwrap().message)
                    }
                    else {
                        Ok(parsed.result)
                    }
                }
            }
            Err(e) => Err(e.to_string())
        }
    }

    /// Get the final unspent balance of an adress represented as a address object
    /// 
    /// # Arguments
    /// 
    /// * `addr` the address for which to check the balance
    pub fn get_address_final_balance_addr(&self, addr : Address) -> Result<u64, String> {
        self.get_address_final_balance(addr.to_string())
    }

    /// Send a signed raw transaction to the Bitcoin Core Node
    /// 
    /// # Arguments
    /// 
    /// * `tx` the signed transaction to send
    pub fn send_raw_transaction(&self, tx : Transaction) -> Result<(), String> {
        let txhex : Vec<u8> = tx.serialize();
        let mut params : Vec<JsonRpcParam> = Vec::new();
        params.push(JsonRpcParam::String(hex::encode(txhex)));
        let rpc = JsonRpc::new(String::from("1.0"), self.settings.id.clone(), String::from("sendrawtransaction"), params);
        let url = self.get_url();
        let req = self.req_factory.new_json_rpc_request(url, rpc, self.settings.user.clone(), self.settings.pass.clone());
        match req.execute() {
            Ok(x) => {
                println!("{}", x.content);   
                let parsed : SendRawTxResponse = serde_json::from_str(&x.content)
                    .expect("Failed to parse sendrawtx rpc response");             
                if parsed.id != self.settings.id {
                    Err("RPC Request and Response id didn't match".to_string())
                }
                else {
                    if parsed.error.is_some() {
                        Err(parsed.error.unwrap().message)
                    }
                    else {
                        Ok(())
                    }
                }
            }
            Err(e) => Err(e.to_string())
        }
    }

    fn get_url(&self) -> String {
        format!("http://{}:{}", self.settings.url, self.settings.port)
    }

}

#[cfg(test)]
mod test {
use bitcoin::Address;

    use crate::bitcoin::btcroutines::create_private_key;
use crate::bitcoin::bitcoin_core::BTC_CORE_RPC_TYPES;
use crate::bitcoin::bitcoin_core::BitcoinCore;
use crate::bitcoin::bitcoin_types::BTCInput;
use crate::net::http::RequestFactory;
use crate::Settings;
use crate::settings::BtcNodeSettings;
use crate::net::http::HttpResponse;
use crate::util;
use std::{fs, str::FromStr};
use crate::bitcoin::bitcoin_core_responses::SendRawTxResponse;
use crate::bitcoin::bitcoin_core_responses::Error;
use crate::bitcoin::btcroutines::create_lock_transaction;


    fn get_btc_core_settings() -> BtcNodeSettings {
        let contents = fs::read_to_string("config/settings.json")
            .expect("Something went wrong reading the settings file");
        let read_settings = Settings::parse_json_string(&contents);
        read_settings.btc
    }

    fn get_mock_response(rtype : BTC_CORE_RPC_TYPES) -> String {
        match rtype {
            BTC_CORE_RPC_TYPES::GET_NETWORK_INFO => {
                String::from(r#"{"result":{"version":200100,"subversion":"/Satoshi:0.20.1/","protocolversion":70015,"localservices":"0000000000000409","localservicesnames":["NETWORK","WITNESS","NETWORK_LIMITED"],"localrelay":true,"timeoffset":0,"networkactive":true,"connections":10,"networks":[{"name":"ipv4","limited":false,"reachable":true,"proxy":"","proxy_randomize_credentials":false},{"name":"ipv6","limited":false,"reachable":true,"proxy":"","proxy_randomize_credentials":false},{"name":"onion","limited":true,"reachable":false,"proxy":"","proxy_randomize_credentials":false}],"relayfee":0.00001000,"incrementalfee":0.00001000,"localaddresses":[],"warnings":"Warning: unknown new rules activated (versionbit 28)"},"error":null,"id":"mw-btc-swap"}"#)
            },
            BTC_CORE_RPC_TYPES::LIST_UNSPENT => {
                String::from(r#"{"result":[{"txid":"211db342720933ca50f4b713c462823f0440f0fc29c3b61ba4f9d1d9e65d4e1b","vout":1,"address":"mtBfJro9QjBV8Wo6vCcsuDcQg4YzsWkLun","label":"","scriptPubKey":"76a9148af38d7889f081a2c1ac5df8121600fdab6dca2788ac","amount":0.00031432,"confirmations":2163,"spendable":false,"solvable":false,"safe":true},{"txid":"794cd697b71c4dbb5ff83c0eb984e6628187daf5d12aa4026c704edc7a667b2b","vout":0,"address":"mtBfJro9QjBV8Wo6vCcsuDcQg4YzsWkLun","label":"","scriptPubKey":"76a9148af38d7889f081a2c1ac5df8121600fdab6dca2788ac","amount":0.00007055,"confirmations":2323,"spendable":false,"solvable":false,"safe":true},{"txid":"d6facfba8c6d15cebda109293a4f37374023d31813bd363b965c2dbdfe04ce3e","vout":0,"address":"mtBfJro9QjBV8Wo6vCcsuDcQg4YzsWkLun","label":"","scriptPubKey":"76a9148af38d7889f081a2c1ac5df8121600fdab6dca2788ac","amount":0.00000546,"confirmations":2145,"spendable":false,"solvable":false,"safe":true},{"txid":"780e458f97de8b622ba254b7f69d027b36b4aabed4f4dea608d5bba17d040975","vout":0,"address":"mtBfJro9QjBV8Wo6vCcsuDcQg4YzsWkLun","label":"","scriptPubKey":"76a9148af38d7889f081a2c1ac5df8121600fdab6dca2788ac","amount":0.09937874,"confirmations":2024,"spendable":false,"solvable":false,"safe":true},{"txid":"3b1dd5732d496974fca2c4434e886f0b9f2996f299929f163377462f02453a7c","vout":0,"address":"mtBfJro9QjBV8Wo6vCcsuDcQg4YzsWkLun","label":"","scriptPubKey":"76a9148af38d7889f081a2c1ac5df8121600fdab6dca2788ac","amount":0.01263314,"confirmations":52,"spendable":false,"solvable":false,"safe":true}],"error":null,"id":"mw-btc-swap"}"#)
            },
            BTC_CORE_RPC_TYPES::SEND_RAW_TRANSACTION => {
                String::from(r#"{"result":"0100000001f3f6a909f8521adb57d898d2985834e632374e770fd9e2b98656f1bf1fdfd427010000006b48304502203a776322ebf8eb8b58cc6ced4f2574f4c73aa664edce0b0022690f2f6f47c521022100b82353305988cb0ebd443089a173ceec93fe4dbfe98d74419ecc84a6a698e31d012103c5c1bc61f60ce3d6223a63cedbece03b12ef9f0068f2f3c4a7e7f06c523c3664ffffffff0260e31600000000001976a914977ae6e32349b99b72196cb62b5ef37329ed81b488ac063d1000000000001976a914f76bc4190f3d8e2315e5c11c59cfc8be9df747e388ac00000000","error":null,"id":"mw-btc-swap"}"#)
            },
            BTC_CORE_RPC_TYPES::GET_BLOCK_COUNT => {
                String::from(r#"{"result":1906786,"error":null,"id":"mw-btc-swap"}"#)
            }
        }
    }

    fn get_mock_err_response(msg : String, code : i32, id : String) -> String {
        let err = Error {
            code : code,
            message : msg
        };
        let r = SendRawTxResponse {
            result : None,
            error : Some(err),
            id : id
        };
        serde_json::to_string(&r).unwrap()
    }

    #[test]
    fn test_getnetwork_info() {
        let stub_response = HttpResponse {
            status : 200,
            content : get_mock_response(BTC_CORE_RPC_TYPES::GET_NETWORK_INFO)
        };
        let factory = RequestFactory::new(Some(stub_response));
        let core = BitcoinCore::new(get_btc_core_settings(), factory);
        let r = core.get_network_info()
            .unwrap();
        assert_eq!(200100, r.version);
        assert_eq!("/Satoshi:0.20.1/", r.subversion);
    }

    #[test]
    fn test_getnetwork_info_invalid_id() {
        let stub_response = HttpResponse {
            status : 200,
            content : get_mock_response(BTC_CORE_RPC_TYPES::GET_NETWORK_INFO).replace("mw-btc-swap", "malicious")
        };
        let factory = RequestFactory::new(Some(stub_response));
        let core = BitcoinCore::new(get_btc_core_settings(), factory);
        assert_eq!(core.get_network_info().err(), Some(String::from("RPC Request and Response id didn't match!")));    
    }

    #[test]
    fn test_getnetwork_info_invalid_status() {
        let stub_response = HttpResponse {
            status : 504,
            content : String::from("")
        };
        let factory = RequestFactory::new(Some(stub_response));
        let core = BitcoinCore::new(get_btc_core_settings(), factory);
        assert_eq!(core.get_network_info().err(), Some(String::from("Failed with invalid respcode")));    
    }

    #[test]
    fn test_getnetwork_info_execution_error() {
        let stub_response = HttpResponse {
            status : 200,
            content : get_mock_err_response("Procedure Execution failed for unexpected reasons".to_string(), -1, "mw-btc-swap".to_string())
        };
        let factory = RequestFactory::new(Some(stub_response));
        let core = BitcoinCore::new(get_btc_core_settings(), factory);
        assert_eq!(core.get_network_info().err(), Some(String::from("Procedure Execution failed for unexpected reasons")));    
    }

    #[test]
    fn test_get_address_balance() {
        let stub_response = HttpResponse {
            status : 200,
            content : get_mock_response(BTC_CORE_RPC_TYPES::LIST_UNSPENT)
        };
        let factory = RequestFactory::new(Some(stub_response));
        let core = BitcoinCore::new(get_btc_core_settings(), factory);
        let b = core.get_address_final_balance(String::from("mtBfJro9QjBV8Wo6vCcsuDcQg4YzsWkLun"))
            .unwrap();
        assert_eq!(11240221, b);
    }

    #[test]
    fn test_get_address_balance_invalidid() {
        let stub_response = HttpResponse {
            status : 200,
            content : get_mock_response(BTC_CORE_RPC_TYPES::LIST_UNSPENT).replace("mw-btc-swap", "malicious")
        };
        let factory = RequestFactory::new(Some(stub_response));
        let core = BitcoinCore::new(get_btc_core_settings(), factory);
        let b = core.get_address_final_balance(String::from("mtBfJro9QjBV8Wo6vCcsuDcQg4YzsWkLun"));
        assert_eq!(b.err(), Some(String::from("RPC Request and Response id didn't match!")));   
    }

    #[test]
    fn test_get_address_balance_invalid_status() {
        let stub_response = HttpResponse {
            status : 504,
            content : String::from("")
        };
        let factory = RequestFactory::new(Some(stub_response));
        let core = BitcoinCore::new(get_btc_core_settings(), factory);
        let b = core.get_address_final_balance(String::from("mtBfJro9QjBV8Wo6vCcsuDcQg4YzsWkLun"));
        assert_eq!(b.err(), Some(String::from("Failed with invalid respcode")));   
    }

    #[test]
    fn test_get_address_balance_error() {
        let stub_response = HttpResponse {
            status : 200,
            content : get_mock_err_response("Procedure Execution failed for unexpected reasons".to_string(), -1, "mw-btc-swap".to_string())
        };
        let factory = RequestFactory::new(Some(stub_response));
        let core = BitcoinCore::new(get_btc_core_settings(), factory);
        let b = core.get_address_final_balance(String::from("mtBfJro9QjBV8Wo6vCcsuDcQg4YzsWkLun"));
        assert_eq!(b.err(), Some(String::from("Procedure Execution failed for unexpected reasons"))); 
    }

    #[test]
    fn test_send_raw_transaction() {
        let stub_response = HttpResponse {
            status : 200,
            content : get_mock_response(BTC_CORE_RPC_TYPES::SEND_RAW_TRANSACTION)
        };
        let factory = RequestFactory::new(Some(stub_response));
        let core = BitcoinCore::new(get_btc_core_settings(), factory);
        let mut rng = util::get_os_rng();
        let curve = util::get_secp256k1_curve();
        let sk = create_private_key(&mut rng);
        let pk = sk.public_key(&curve);
        let inp = vec!(BTCInput{
            txid : "2621c2609d114f652dadf6fd95820c021da1cf2d0ac15e0361fd5d136e30a3c4".to_string(),
            vout : 0,
            value : 10000,
            secret : "b72c00b3e55433d3266d184cf8a8916892aba28023cd46db617f36cac1ebcf8e".to_string(),
            pub_key : "b72c00b3e55433d3266d184cf8a8916892aba28023cd46db617f36cac1ebcf8e".to_string(),
            pub_script : "b72c00b3e55433d3266d184cf8a8916892aba28023cd46db617f36cac1ebcf8e".to_string()
        });
        let tx = create_lock_transaction(pk, pk, pk, pk,inp, 1000, 1, 50000)
            .unwrap();
        let r = core.send_raw_transaction(tx);
        assert_eq!(Ok(()), r);
    }

    #[test]
    fn test_send_raw_transaction_invalidid() {
        let stub_response = HttpResponse {
            status : 200,
            content : get_mock_response(BTC_CORE_RPC_TYPES::SEND_RAW_TRANSACTION).replace("mw-btc-swap", "malicious")
        };
        let factory = RequestFactory::new(Some(stub_response));
        let core = BitcoinCore::new(get_btc_core_settings(), factory);
        let mut rng = util::get_os_rng();
        let curve = util::get_secp256k1_curve();
        let sk = create_private_key(&mut rng);
        let pk = sk.public_key(&curve);
        let inp = vec!(BTCInput{
            txid : "2621c2609d114f652dadf6fd95820c021da1cf2d0ac15e0361fd5d136e30a3c4".to_string(),
            vout : 0,
            value : 10000,
            secret : "b72c00b3e55433d3266d184cf8a8916892aba28023cd46db617f36cac1ebcf8e".to_string(),
            pub_key : "b72c00b3e55433d3266d184cf8a8916892aba28023cd46db617f36cac1ebcf8e".to_string(),
            pub_script : "b72c00b3e55433d3266d184cf8a8916892aba28023cd46db617f36cac1ebcf8e".to_string()
        });
        let tx = create_lock_transaction(pk, pk, pk, pk, inp, 1000, 1, 50000)
            .unwrap();
        let r = core.send_raw_transaction(tx);
        assert_eq!(r.err(), Some(String::from("RPC Request and Response id didn't match"))); 
    }

    #[test]
    fn test_send_raw_transaction_error() {
        let stub_response = HttpResponse {
            status : 200,
            content : get_mock_err_response("Invalid inputs".to_string(), -32, "mw-btc-swap".to_string())
        };
        let factory = RequestFactory::new(Some(stub_response));
        let core = BitcoinCore::new(get_btc_core_settings(), factory);
        let mut rng = util::get_os_rng();
        let curve = util::get_secp256k1_curve();
        let sk = create_private_key(&mut rng);
        let pk = sk.public_key(&curve);
        let inp = vec!(BTCInput{
            txid : "2621c2609d114f652dadf6fd95820c021da1cf2d0ac15e0361fd5d136e30a3c4".to_string(),
            vout : 0,
            value : 10000,
            secret : "b72c00b3e55433d3266d184cf8a8916892aba28023cd46db617f36cac1ebcf8e".to_string(),
            pub_key : "b72c00b3e55433d3266d184cf8a8916892aba28023cd46db617f36cac1ebcf8e".to_string(),
            pub_script : "b72c00b3e55433d3266d184cf8a8916892aba28023cd46db617f36cac1ebcf8e".to_string()
        });
        let tx = create_lock_transaction(pk, pk, pk, pk, inp, 1000, 1, 50000)
            .unwrap();
        let r = core.send_raw_transaction(tx);
        assert_eq!(r.err(), Some(String::from("Invalid inputs")));
    }

    #[test]
    fn test_block_count() {
        let stub_response = HttpResponse {
            status : 200,
            content : get_mock_response(BTC_CORE_RPC_TYPES::GET_BLOCK_COUNT)
        };
        let factory = RequestFactory::new(Some(stub_response));
        let core = BitcoinCore::new(get_btc_core_settings(), factory);
        let count = core.get_current_block_height().unwrap();
        assert_eq!(1906786, count);
    }

    #[test]
    fn test_import_address() {
        let stub_response = HttpResponse {
            status : 200,
            content : String::from(r#"{"result":null,"error":null,"id":"resttest"}"#)
        };
        let factory = RequestFactory::new(Some(stub_response));
        let core = BitcoinCore::new(get_btc_core_settings(), factory);
        let addr = Address::from_str(&String::from("myBqUUBtJ1H3cZtczAo9t6VT4yEXQaMSnU")).unwrap();
        core.import_btc_address(addr).unwrap();
    }
}