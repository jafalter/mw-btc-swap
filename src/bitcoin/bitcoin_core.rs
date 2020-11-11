use serde::{Serialize, Deserialize};
use crate::settings::BtcNodeSettings;

pub struct BitcoinCore {
    settings : BtcNodeSettings
}

#[derive(Serialize, Deserialize)]
pub struct JsonRpc {
    jsonrpc : String,
    id : String,
    method : String,
    params : Vec<String>
}

impl BitcoinCore {
    /// Creates a new object of the BitcoinCore struct which can
    /// be used to send RPC request to a BitcoinCore node
    /// 
    /// # Arguments
    /// 
    /// * `settings` Connection settings (url, port, authentication)
    pub fn new(settings : BtcNodeSettings) -> BitcoinCore {
        BitcoinCore {
            settings
        }
    }

    /// Builds a new JSON RPC v1.0 request which 
    /// can be send to a Bitcoin Core node
    /// 
    /// # Arguments
    /// 
    /// * `id` some arbitrarily chosen request id
    /// * `method` the method to execute on the Bitcoin Core node
    /// * `params` parameters passed to the Bitcoin Core node 
    fn getRequest(&self, id : String, method : String, params : Vec<String>) -> JsonRpc {
        JsonRpc {
            jsonrpc : String::from("1.0"),
            id : id,
            method : method,
            params : params
        }
    }

    pub fn import_address(&self, address : String) {
        
    }
}