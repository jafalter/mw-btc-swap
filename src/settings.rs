use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct Settings {
    pub btc_node_url: String,
    pub mw_node_url: String
}

impl Settings {
    // Parse JSON string
    pub fn parse_json_string(json : &str) -> Settings {
        let s : Settings = serde_json::from_str(&json).unwrap();
        s
    }
}