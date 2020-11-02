use crate::grin::grin_types::MWCoin;
use crate::swap::slate::read_slate_from_disk;
use rand::rngs::OsRng;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::All;
use crate::SwapSlate;
use crate::Settings;
use crate::commands::cmd_types::command::Command;

pub struct ImportGrin {
    swpid : u64,
    commitment : String,
    blinding_factor : String,
    value : u64
}

impl ImportGrin {
    pub fn new(swpid : u64, commitment : String, blinding_factor : String, value : u64) -> ImportGrin {
        ImportGrin {
            swpid : swpid,
            commitment : commitment,
            blinding_factor : blinding_factor,
            value : value
        }
    }
}

impl Command for ImportGrin {
    fn execute(&self, settings : Settings, rng : &mut OsRng, curve : &Secp256k1<All>) -> Result<SwapSlate, &'static str> {
        let mut slate : SwapSlate = read_slate_from_disk(self.swpid, settings.slate_directory.clone()).expect("Failed to read SwapSlate from file");
        slate.prv_slate.mw.inputs.push(MWCoin{
            commitment : self.commitment.clone(),
            blinding_factor : self.blinding_factor.clone(),
            value : self.value
        });
        Ok(slate)
    }
}