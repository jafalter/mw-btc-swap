use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct BTCInput {
    pub txid: String,
    pub vout: u32,
    pub value: u64,
    pub secret: String, // in WIF
    pub pub_key: String,
    pub pub_script: String
}

impl BTCInput {

    pub fn new(txid : String, vout : u32, value : u64, secret: String, pub_key: String, pub_script: String) -> BTCInput {
        BTCInput {
            txid : txid,
            vout : vout,
            value : value,
            secret : secret,
            pub_key : pub_key,
            pub_script : pub_script,
        }
    }
}