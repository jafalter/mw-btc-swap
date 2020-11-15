use crate::net::http::JsonRpc;
use crate::settings::BtcNodeSettings;

pub struct BitcoinCore {
    settings : BtcNodeSettings
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

    pub fn import_address(&self, address : String) {
        
    }
}