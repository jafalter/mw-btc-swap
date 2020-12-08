use crate::net::http::RequestFactory;
use crate::net::http::JsonRpc;
use crate::settings::BtcNodeSettings;
use crate::bitcoin::bitcoin_core_responses::NetworkInfo;
use crate::bitcoin::bitcoin_core_responses::NetworkInfoResult;
use crate::bitcoin::bitcoin_core_responses::ListUnspentResponse;
use bitcoin::util::address::Address;

pub struct BitcoinCore {
    settings : BtcNodeSettings,
    req_factory : RequestFactory
}

pub enum BTC_CORE_RPC_TYPES {
    GET_NETWORK_INFO,
    LIST_UNSPENT
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
        let rpc = JsonRpc::new(String::from("1.0"), self.settings.id.clone(), String::from("getnetworkinfo"), String::from("[]"));
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
                        Err(parsed.error.unwrap())
                    }
                    else {
                        Ok(parsed.result)
                    }
                }
            },
            Err(e) => Err(e.to_string())
        }
    }

    /// Import a new address into the node to be able to check its balance
    /// Call to this method takes very long as it triggers a rescan therefore we expect it to timeout
    /// 
    /// # Arguments
    /// 
    /// * `addr` the address to index
    pub fn import_btc_address(&self, addr : Address) {
        let rpc = JsonRpc::new(String::from("1.0"), self.settings.id.clone(), String::from("importaddress"), format!("{}", addr.to_string()));
        let url = format!("http://{}:{}", self.settings.url, self.settings.port);
        let req = self.req_factory.new_json_rpc_request(url, rpc, self.settings.user.clone(), self.settings.pass.clone());
        match req.execute() {
            _ => ()
        }
    }

    /// Get the final unspent balance of an address represented as a string
    /// 
    /// # Arguments
    /// 
    /// * `addr` the address for which to check the balance
    pub fn get_address_final_balance(&self, addr: String) -> Result<u64, String> {
        let params : String = format!(r#"[1,9999999,["{}"]]"#, addr);
        let rpc = JsonRpc::new(String::from("1.0"), self.settings.id.clone(), String::from("listunspent"), params);
        let url = self.get_url();
        let req = self.req_factory.new_json_rpc_request(url, rpc, self.settings.user.clone(), self.settings.pass.clone());
        match req.execute() {
            Ok(x) => {
                println!("{}", x.content);
                let parsed : ListUnspentResponse = serde_json::from_str(&x.content)
                    .expect("Failed to parse listunspent rpc response");
                if parsed.id != self.settings.id {
                    Err("RPC Request and Resposne id didn't match!".to_string())
                }
                else {
                    if parsed.error.is_some() {
                        Err(parsed.error.unwrap())
                    }
                    else {
                        let mut balance : u64 = 0;
                        // Sum up the unspent balances of the UTXOs under this address
                        for e in &parsed.result {
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

    /// Get the final unspent balance of an adress represented as a address object
    /// 
    /// # Arguments
    /// 
    /// * `addr` the address for which to check the balance
    pub fn get_address_final_balance_addr(&self, addr : Address) -> Result<u64, String> {
        self.get_address_final_balance(addr.to_string())
    }

    fn get_url(&self) -> String {
        format!("http://{}:{}", self.settings.url, self.settings.port)
    }

}

#[cfg(test)]
mod test {
    use crate::bitcoin::bitcoin_core::BTC_CORE_RPC_TYPES;
use crate::bitcoin::bitcoin_core::BitcoinCore;
use crate::net::http::RequestFactory;
    use crate::Settings;
    use crate::settings::BtcNodeSettings;
    use crate::net::http::HttpResponse;
    use std::fs;

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
            }
        }
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
            content : get_mock_response(BTC_CORE_RPC_TYPES::LIST_UNSPENT).replace("null", "\"Procedure Execution failed for unexpected reasons\"")
        };
        let factory = RequestFactory::new(Some(stub_response));
        let core = BitcoinCore::new(get_btc_core_settings(), factory);
        let b = core.get_address_final_balance(String::from("mtBfJro9QjBV8Wo6vCcsuDcQg4YzsWkLun"));
        assert_eq!(b.err(), Some(String::from("Procedure Execution failed for unexpected reasons"))); 
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
            content : get_mock_response(BTC_CORE_RPC_TYPES::GET_NETWORK_INFO).replace("null", "\"Procedure Execution failed for unexpected reasons\"")
        };
        let factory = RequestFactory::new(Some(stub_response));
        let core = BitcoinCore::new(get_btc_core_settings(), factory);
        assert_eq!(core.get_network_info().err(), Some(String::from("Procedure Execution failed for unexpected reasons")));    
    }

}