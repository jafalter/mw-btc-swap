use bitcoin::secp256k1::Secp256k1;
use rand::rngs::OsRng;
use bitcoin::secp256k1::All;
use grin_util::secp::Secp256k1 as GrinSecp256k1;

use crate::{commands::cmd_types::command::Command, settings::Settings, swap::swap_types::SwapSlate};
pub struct Cancel {
    swapid : u64
}

impl Cancel {
    pub fn new(swapid : u64) -> Cancel {
        Cancel {
            swapid : swapid
        }
    }
}

impl Command for Cancel {
    fn execute(&self, settings : &Settings, rng : &mut OsRng, btc_secp : &Secp256k1<All>, grin_secp : &GrinSecp256k1) -> Result<SwapSlate, String> {
        // TODO implement
        Err(String::from("Not implemented"))
    }
}