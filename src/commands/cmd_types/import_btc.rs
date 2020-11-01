use crate::bitcoin::bitcoin_types::BTCInput;
use crate::swap::slate::read_slate_from_disk;
use rand::rngs::OsRng;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::All;
use crate::SwapSlate;
use crate::Settings;
use crate::commands::cmd_types::command::Command;
use bitcoin::util::key::PrivateKey;

pub struct ImportBtc {
    swpid : u64,
    txid : String,
    vout : u16,
    value : u64,
    secret: String
}

impl ImportBtc {
    pub fn new(swpid : u64, txid : String, vout : u16, value : u64, secret : String) -> ImportBtc {
        ImportBtc {
            swpid : swpid,
            txid : txid,
            vout : vout,
            value : value,
            secret : secret
        }
    }
}

impl Command for ImportBtc {
    fn execute(&self, settings: Settings, rng : &mut OsRng, curve : &Secp256k1<All>) -> Result<SwapSlate, &'static str> {
        let mut slate : SwapSlate = read_slate_from_disk(self.swpid, settings.slate_directory.clone()).expect("Failed to read SwapSlate");
        let sec_key = PrivateKey::from_wif(&self.secret).expect("Unable to parse private key, please provide in WIF format");
        slate.prv_slate.btc.inputs.push(BTCInput{
            txid : self.txid.clone(),
            vout : self.vout,
            value : self.value,
            secret : sec_key.to_wif()
        });
        Ok(slate)
    }
}