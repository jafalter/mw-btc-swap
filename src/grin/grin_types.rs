use serde::{Serialize, Deserialize};
use grin_util::secp::key::SecretKey;
use grin_util::secp::pedersen::Commitment;
use crate::grin::grin_routines::{*};

#[derive(Serialize, Deserialize)]
pub struct MWCoin {
    pub commitment : String,
    pub blinding_factor: String,
    pub value: u64
}

impl MWCoin {
    /// Create a new Grin spendable coin
    /// # Arguments
    ///
    /// * `commitment` the coin pedersen commitment
    /// * `blinding_factor` blinding factor used in the commitment
    /// * `value` coin value as nanogrins
    pub fn new(commitment : &Commitment, blinding_factor : &SecretKey, value : u64) -> MWCoin {
        let enc_com = serialize_commitment(commitment);
        let enc_bf = serialize_secret_key(blinding_factor);
        MWCoin {
            commitment : enc_com,
            blinding_factor : enc_bf,
            value : value
        }
    }
}