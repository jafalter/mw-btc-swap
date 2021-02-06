use bitcoin::PublicKey;
use crate::bitcoin::bitcoin_types::BTCInput;
use crate::swap::slate::read_slate_from_disk;
use rand::rngs::OsRng;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::All;
use crate::SwapSlate;
use crate::Settings;
use crate::commands::cmd_types::command::Command;
use bitcoin::util::key::PrivateKey;
use grin_util::secp::Secp256k1 as GrinSecp256k1;
pub struct ImportBtc {
    swpid : u64,
    txid : String,
    vout : u32,
    value : u64,
    sk: String,
    pub_script: String,
}

impl ImportBtc {
    pub fn new(swpid : u64, txid : String, vout : u32, value : u64, sk : String, pub_script : String) -> ImportBtc {
        ImportBtc {
            swpid : swpid,
            txid : txid,
            vout : vout,
            value : value,
            sk : sk,
            pub_script : pub_script
        }
    }
}

impl Command for ImportBtc {
    fn execute(&self, settings: &Settings, rng : &mut OsRng, btc_secp : &Secp256k1<All>, grin_secp : &GrinSecp256k1) -> Result<SwapSlate, String> {
        let mut slate : SwapSlate = read_slate_from_disk(self.swpid, &settings.slate_directory)
            .expect("Failed to read SwapSlate");
        let sec_key = PrivateKey::from_wif(&self.sk)
            .expect("Unable to parse private key, please provide in WIF format");
        let pub_key = PublicKey::from_private_key(btc_secp, &sec_key);
        slate.prv_slate.btc.inputs.push(BTCInput{
            txid : self.txid.clone(),
            vout : self.vout,
            value : self.value,
            secret : sec_key.to_wif(),
            pub_key : pub_key.to_string(),
            pub_script: self.pub_script.clone()
        });
        Ok(slate)
    }
}