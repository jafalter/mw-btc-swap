use crate::enums::SwapStatus;
use crate::grin::grin_types::MWCoin;
use crate::bitcoin::bitcoin_types::BTCInput;

pub struct SwapSlate {
    pub id : u64,
    pub pubSlate : SwapSlatePub,
    pub privSlate : SwapSlatePriv
}

pub struct SwapSlatePub {
    pub status : SwapStatus,
    pub mw : MWPub,
    pub btc : BTCPub,
    pub meta : Meta
}

pub struct SwapSlatePriv {
    pub mw : MWPriv,
    pub btc: BTCPriv
}

pub struct Meta {
    pub server : String,
    pub port : String
}

pub struct MWPub {
    pub amount : u64,
    pub timelock : u32
}

pub struct MWPriv {
    pub inputs : Vec<MWCoin>,
    pub partial_key : u64,
}

pub struct BTCPub {
    pub amount : u64,
    pub timelock : u32,
    pub stmt : Option<String>
}

pub struct BTCPriv {
    pub inputs : Vec<BTCInput>,
    pub witness : u64
}