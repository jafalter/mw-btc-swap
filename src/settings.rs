use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct BtcNodeSettings {
    pub url : String,
    pub user : String,
    pub pass : String,
    pub port : u16,
    pub id : String
}

impl BtcNodeSettings {
    pub fn clone(&self) -> BtcNodeSettings {
        BtcNodeSettings {
            url : self.url.clone(),
            user : self.user.clone(),
            pass : self.pass.clone(),
            port : self.port,
            id : self.id.clone()
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Settings {
    pub btc: BtcNodeSettings,
    pub mw_node_url: String,
    pub tcp_addr: String,
    pub tcp_port : String,
    pub slate_directory : String,
}

impl Settings {
    // Parse JSON string
    pub fn parse_json_string(json : &str) -> Settings {
        let s : Settings = serde_json::from_str(&json).unwrap();
        s
    }
}