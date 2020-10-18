use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct MWCoin {
    pub commitment : String,
    pub blinding_factor: String,
    pub value: u64
}