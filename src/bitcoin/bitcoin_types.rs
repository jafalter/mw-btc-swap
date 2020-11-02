use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct BTCInput {
    pub txid: String,
    pub vout: u32,
    pub value: u64,
    pub secret: String,
    pub pub_key: String,
    pub pub_script: String
}