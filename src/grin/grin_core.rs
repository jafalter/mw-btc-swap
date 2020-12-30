use crate::grin::grin_types::MWCoin;
use grin_wallet_libwallet::Slate;
use grin_util::secp::pedersen::Commitment;
use grin_core::core::transaction::OutputFeatures;
use grin_core::core::Input;
use grin_core::core::Inputs;
use grin_core::core::Output;
use grin_core::core::Transaction;
use rand::rngs::OsRng;
use grin_util::secp::Secp256k1;
use grin_util::secp::ContextFlag;
use grin_util::secp::key::{PublicKey, SecretKey};
use grin_core::core::transaction::FeeFields;
use grin_core::core::OutputIdentifier;
use grin_wallet_libwallet::Context;
use grin_keychain::{Keychain,ExtKeychain,Identifier};
use crate::util::get_os_rng;
use crate::grin::grin_routines::{*};

pub struct GrinCore {
    rng : OsRng,
    secp : Secp256k1,
    chain : ExtKeychain
}

pub struct SpendCoinsResult {
    slate : Slate,
    sig_key : SecretKey,
    sig_nonce : SecretKey,
    change_coin : MWCoin
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

    pub fn spend_coins(&mut self, inputs : Vec<MWCoin>, fund_value : u64, fee : u64, timelock : u32, num_participants : u8) -> Result<SpendCoinsResult,String> {
        // Initial transaction slate
        let mut mwslate = Slate::blank(num_participants, false);
        
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
            let out_bf = create_secret_key(&mut self.rng, &self.secp);
            let out_bf_com = out_bf.clone();
            let out_bf_prf = out_bf.clone();
            let sig_nonce = create_secret_key(&mut self.rng, &self.secp);
            let prf_nonce = create_secret_key(&mut self.rng, &self.secp);
            let rew_nonce = create_secret_key(&mut self.rng, &self.secp);
            let mut bf_sum = out_bf.clone();

            mwslate.amount = fund_value;
            let feefield = FeeFields::new(0, fee).unwrap();
            mwslate.fee_fields = feefield;
            let mut tx = Transaction::empty();
    
            // Add the input coins
            let mut inp_vector : Vec<Input> = vec!();
            for coin in inputs {
                let commitment = deserialize_commitment(&coin.commitment);
                let mut inp_bf = deserialize_secret_key(&coin.blinding_factor, &self.secp);
                let input = Input::new(OutputFeatures::Plain, commitment);
                inp_vector.push(input);
                inp_bf.inv_assign(&self.secp).unwrap();
                bf_sum.add_assign(&self.secp, &inp_bf).unwrap();
                tx = tx.with_input(input);
            }
            let inputs = Inputs::FeaturesAndCommit(inp_vector);
            tx = Transaction::new(inputs, &tx.body.outputs, tx.body.kernels());
    
            // Add changecoin output
            let change_value = inpval - fund_value - fee;
            let commitment = self.secp.commit(change_value, out_bf_com)
                .expect("Failed to genere pedersen commitment for change output coin");
            // Compute bulletproof rangeproof
            let proof = self.secp.bullet_proof(change_value, out_bf_prf, rew_nonce, prf_nonce, None, None);
            let output = Output::new(OutputFeatures::Plain, commitment, proof);
            tx = tx.with_output(output);
            mwslate.tx = Some(tx);
            let mut ctx : Context = Context{
                parent_key_id: Identifier::zero(),
                sec_key: bf_sum.clone(),
                sec_nonce: sig_nonce.clone(),
                initial_sec_key: out_bf.clone(),
                initial_sec_nonce: sig_nonce.clone(),
                output_ids: vec!(),
                input_ids: vec!(),
                amount: fund_value,
                fee: Some(feefield),
                payment_proof_derivation_index: None,
                late_lock_args: None,
                calculated_excess: None,
            };
            mwslate.fill_round_1(&self.chain, &mut ctx)
                .expect("Failed to add senders particpant data");
            let enc_com = serialize_commitment(&commitment);
            let enc_bf = serialize_secret_key(&out_bf);
    
            Ok(SpendCoinsResult {
                slate : mwslate,
                sig_key : bf_sum.clone(),
                sig_nonce : sig_nonce.clone(),
                change_coin : MWCoin {
                    commitment : enc_com,
                    blinding_factor: enc_bf,
                    value: change_value
                }
            })
        }
    }

    pub fn recv_coins(&mut self, ptx : Slate, fund_value : u64) {
        // Validate output coin rangeproofs
        for out in ptx.tx.unwrap().outputs() {
            let prf = out.proof;
            let com = out.identifier.commit;
            let vrf = self.secp.verify_bullet_proof(com, prf, None)
                .expect("Failed to verify outputcoin rangeproof");
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
        println!("{}", result.change_coin.commitment);
    }
}