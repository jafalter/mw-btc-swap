use crate::net::http::RequestFactory;
use crate::net::http::JsonRpc;
use crate::settings::BtcNodeSettings;
use crate::bitcoin::bitcoin_core_responses::NetworkInfo;
use crate::bitcoin::bitcoin_core_responses::NetworkInfoResult;

pub struct BitcoinCore {
    settings : BtcNodeSettings,
    req_factory : RequestFactory
}

pub enum BTC_CORE_RPC_TYPES {
    GET_NETWORK_INFO
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
        let rpc = JsonRpc::new(String::from("1.0"), self.settings.id.clone(), String::from("getnetworkinfo"), Vec::new());
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