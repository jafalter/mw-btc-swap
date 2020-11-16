use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct NetworkInfo {
    pub result : NetworkInfoResult,
    pub error : Option<String>,
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