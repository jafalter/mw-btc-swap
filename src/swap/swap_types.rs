use crate::enums::SwapType;
use bitcoin::{PrivateKey, PublicKey};
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
    pub timelock : u64,
    pub lock_time : Option<i64>,
    pub swap_type : SwapType
}

#[derive(Serialize, Deserialize)]
pub struct MWPriv {
    pub inputs : Vec<MWCoin>,
    pub partial_key : u64,
    pub shared_coin : Option<MWCoin>,
    pub refund_coin : Option<MWCoin>,
    pub swapped_coin : Option<MWCoin>
}

#[derive(Serialize, Deserialize)]
pub struct BTCPub {
    pub amount : u64,
    pub timelock : u64,
    pub swap_type : SwapType,
    pub lock_time : Option<i64>,
    pub pub_a : Option<String>,
    pub pub_b : Option<String>,
    pub pub_x : Option<String>,
    pub lock : Option<BTCInput>,
}

#[derive(Serialize, Deserialize)]
pub struct BTCPriv {
    pub inputs : Vec<BTCInput>,
    pub witness : u64,
    pub sk : Option<String>,
    pub x : Option<String>,
    pub r_sk : Option<String>,
    pub swapped : Option<BTCInput>
}