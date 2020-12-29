use crate::grin::grin_types::MWCoin;
use grin_wallet_libwallet::Slate;
use grin_util::secp::pedersen::{Commitment};
use grin_core::core::transaction::OutputFeatures;
use grin_core::core::Input;
use grin_core::core::Output;
use rand::rngs::OsRng;
use grin_util::secp::Secp256k1;
use grin_util::secp::ContextFlag;
use grin_util::secp::key::{PublicKey, SecretKey};
use grin_wallet_libwallet::ParticipantData;
use grin_keychain::ExtKeychain;
use grin_keychain::Keychain;
use crate::util::get_os_rng;
use crate::grin::grin_routines::create_secret_key;
use crate::grin::grin_routines::clone_secret_key;

pub struct GrinCore {
    rng : OsRng,
    secp : Secp256k1,
    chain : ExtKeychain
}

pub struct SpendCoinsResult {
    slate : Slate,
    sig_key : SecretKey,
    sig_nonce : SecretKey,
    out_bf : SecretKey
}

impl GrinCore {

    pub fn new() -> GrinCore {
        let rng = get_os_rng();
        let secp = Secp256k1::with_caps(ContextFlag::Commit);
        let keychain = ExtKeychain::from_random_seed(true).unwrap();
        GrinCore {
            rng : rng,
            secp : secp,
            chain : keychain
        }
    }

    pub fn spend_coins(&mut self, inputs : Vec<MWCoin>, fund_value : u64, fee : u64, timelock : u32, num_participants : usize) -> Result<SpendCoinsResult,String> {
        // Initial transaction slate
        let mut mwslate = Slate::blank(num_participants);
        
        // Some input param validations
        let mut inpval : u64 = 0;
        let mut duplicate = false;
        for (i,coin) in inputs.iter().enumerate() {
            inpval = inpval + coin.value;
            for (j,cmp) in inputs.iter().enumerate() {
                if i != j && coin.commitment == cmp.commitment {
                    duplicate = true;
                }
            }
        }
        if inpval < (fund_value + fee) {
            Err(String::from("Spend coins function failed, input coins do not have enough value"))
        }
        else if duplicate {
            Err(String::from("Spend coins function failed, duplicate input coins provided"))
        }
        else {
            // Create needed blinding factors and nonce values
            let mut out_bf = create_secret_key(&mut self.rng, &self.secp);
            let out_bf_com = clone_secret_key(&out_bf);
            let out_bf_prf = clone_secret_key(&out_bf);
            let sig_nonce = create_secret_key(&mut self.rng, &self.secp);
            let prf_nonce = create_secret_key(&mut self.rng, &self.secp);
            let rew_nonce = create_secret_key(&mut self.rng, &self.secp);
            let mut bf_sum = clone_secret_key(&out_bf);

            println!("{}", hex::encode(&out_bf.0));

            mwslate.amount = fund_value;
            mwslate.fee = fee;
    
            // Add the input coins
            for coin in inputs {
                let commitment = Commitment::from_vec(hex::decode(coin.commitment).expect("Failed to decode commitment of input"));
                let input = Input {
                    features : OutputFeatures::Plain,
                    commit : commitment
                };
                let mut inp_bf = SecretKey::from_slice(&self.secp, &hex::decode(coin.blinding_factor).unwrap()).unwrap();
                inp_bf.inv_assign(&self.secp).unwrap();
                bf_sum.add_assign(&self.secp, &inp_bf).unwrap();
                mwslate.tx.body.inputs.push(input);
            }
    
            // Add changecoin output
            let change_value = inpval - fund_value - fee;
            let commitment = self.secp.commit(change_value, out_bf_com)
                .expect("Failed to genere pedersen commitment for change output coin");
            // Compute bulletproof rangeproof
            let proof = self.secp.bullet_proof(change_value, out_bf_prf, rew_nonce, prf_nonce, None, None);
            let output = Output {
                features : OutputFeatures::Plain,
                commit : commitment,
                proof : proof
            };
            mwslate.tx.body.outputs.push(output);
            mwslate.fill_round_1(&self.chain, &mut bf_sum, &sig_nonce, 0, None, false)
                .expect("Failed to add senders particpant data");
    
            Ok(SpendCoinsResult {
                slate : mwslate,
                sig_key : clone_secret_key(&bf_sum),
                sig_nonce : clone_secret_key(&sig_nonce),
                out_bf : clone_secret_key(&out_bf)
            })
        }
    }
    
}

#[cfg(test)]
mod test {
    use crate::grin::grin_types::MWCoin;
    use crate::grin::grin_core::GrinCore;
    
    #[test]
    fn test_spend_coins() {
        let mut core = GrinCore::new();
        let coin = MWCoin {
            commitment : String::from("086061571ea044365c81b5232c261866265024bd5c3506b5526d80df0c6c5845c8"),
            blinding_factor : String::from("1682c7950a19dfbc2f2c409ff9517cc72d2bf1ffa9ab1f83746e843461e1112c"),
            value : 50000
        };
        let result = core.spend_coins(vec!(coin), 600, 20, 0, 2).unwrap();
        let ser = serde_json::to_string(&result.slate).unwrap();
        println!("{}", ser);
    }
}