use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct NetworkInfo {
    pub result : Option<NetworkInfoResult>,
    pub error : Option<Error>,
    pub id : String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NetworkInfoResult {
    pub version : u32,
    pub subversion : String,
    pub localservices : String,
    pub localservicesnames : Vec<String>,
    pub localrelay : bool,
    pub timeoffset : u32,
    pub networkactive : bool,
    pub connections : u32,
    pub relayfee : f32,
    pub incrementalfee : f32,
    pub warnings : String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ListUnspentResponse {
    pub result : Option<Vec<UTXO>>,
    pub error : Option<Error>,
    pub id : String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BlockCountResponse {
    pub result : u64,
    pub error : Option<Error>,
    pub id : String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SendRawTxResponse {
    pub result : Option<String>,
    pub error : Option<Error>,
    pub id : String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Error {
    pub code : i32,
    pub message : String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UTXO {
    pub txid : String,
    pub vout : u32,
    pub address : String,
    pub label : String,
    pub scriptPubKey : String,
    pub amount : f32,
    pub confirmations : u32,
    pub spendable : bool,
    pub solvable : bool,
    pub safe : bool
}