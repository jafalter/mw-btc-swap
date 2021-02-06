use crate::swap::slate::create_priv_from_pub;
use crate::SwapSlate;
use rand::rngs::OsRng;
use crate::Settings;
use crate::commands::cmd_types::command::Command;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::All;
use grin_util::secp::Secp256k1 as GrinSecp256k1;

/// Accept command allows a peer to accept the public slate file from an offerer
pub struct Accept {
    swapid : u64
}

impl Accept {
    pub fn new(swapid : u64) -> Accept {
        Accept {
            swapid : swapid
        }
    }
}

impl Command for Accept {
    fn execute(&self, settings : &Settings, rng : &mut OsRng, btc_secp : &Secp256k1<All>, grin_secp : &GrinSecp256k1) -> Result<SwapSlate, String> {
        let slate : SwapSlate = create_priv_from_pub(self.swapid, &settings.slate_directory)
            .expect("Unable to locate public slate file");
        println!("Created private slate file for {}", self.swapid);
        println!("Please import your inputs before starting the swap");
        Ok(slate)
    }
}