use serde::{Serialize, Deserialize};

// {
//"id": 1,
//"jsonrpc": "2.0",
//"result": {
//"Ok": {
//"height": 696667,
//"last_block_pushed": "721ba5eae7dd2c0ae9fa081d8b4efc335950ad62e59938b2ee326ad53944eb1b",
//"prev_block_to_last": "0113cd1949b4880e7bc4af7360341f4a68a78383974b54046f4ff164f2fc9c51",
//"total_difficulty": 1117561204219
//}
//}
//}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetTipResponseOk {
}