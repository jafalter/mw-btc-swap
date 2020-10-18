use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct BTCInput {
    pub txid: String,
    pub vout: u16,
    pub value: u64,
    pub secret: String
}