use crate::enums::SwapType;
use serde::{Serialize, Deserialize};

use crate::enums::SwapStatus;
use crate::grin::grin_types::MWCoin;
use crate::bitcoin::bitcoin_types::BTCInput;

pub struct SwapSlate {
    pub id : u64,
    pub pub_slate : SwapSlatePub,
    pub prv_slate : SwapSlatePriv
}

#[derive(Serialize, Deserialize)]
pub struct SwapSlatePub {
    pub status : SwapStatus,
    pub mw : MWPub,
    pub btc : BTCPub,
    pub meta : Meta
}

#[derive(Serialize, Deserialize)]
pub struct SwapSlatePriv {
    pub mw : MWPriv,
    pub btc: BTCPriv
}

#[derive(Serialize, Deserialize)]
pub struct Meta {
    pub server : String,
    pub port : String
}

#[derive(Serialize, Deserialize)]
pub struct MWPub {
    pub amount : u64,
    pub timelock : u32,
    pub swap_type : SwapType
}

#[derive(Serialize, Deserialize)]
pub struct MWPriv {
    pub inputs : Vec<MWCoin>,
    pub partial_key : u64,
}

#[derive(Serialize, Deserialize)]
pub struct BTCPub {
    pub amount : u64,
    pub timelock : u32,
    pub swap_type : SwapType,
    pub stmt : Option<String>
}

#[derive(Serialize, Deserialize)]
pub struct BTCPriv {
    pub inputs : Vec<BTCInput>,
    pub witness : u64
}