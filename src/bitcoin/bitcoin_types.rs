use bitcoin::{PrivateKey, PublicKey, Script};
use serde::{Serialize, Deserialize};

use super::btcroutines::{serialize_priv_key, serialize_pub_key, serialize_script};

#[derive(Serialize, Deserialize, Clone)]
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

    pub fn new2(txid : String, vout : u32, value : u64, secret: PrivateKey, pub_key : PublicKey, pub_script: Script) -> BTCInput {
        BTCInput {
            txid : txid,
            vout : vout,
            value : value,
            secret : serialize_priv_key(&secret),
            pub_key : serialize_pub_key(&pub_key),
            pub_script : serialize_script(&pub_script)
        }
    }
}