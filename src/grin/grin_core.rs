use crate::grin::grin_types::MWCoin;
use grin_wallet_libwallet::Slate;
use grin_util::secp::pedersen::Commitment;
use grin_core::core::transaction::OutputFeatures;
use grin_core::core::{Input,Inputs,Output,Transaction};
use rand::rngs::OsRng;
use grin_util::secp::{Secp256k1,ContextFlag};
use grin_util::secp::key::{PublicKey, SecretKey};
use grin_core::libtx::tx_fee;
use grin_core::core::transaction::FeeFields;
use grin_wallet_libwallet::Context;
use grin_keychain::{Keychain,ExtKeychain,Identifier,BlindingFactor,BlindSum};
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

pub struct RecvCoinsResult {
    slate : Slate,
    output_coin : MWCoin
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

    /// Implementation of the spend_coins algorithm outlined in the thesis
    /// Called by the sender to initiate a transaction protocol 
    /// Returs a pre-transaction, signing keys and the newly created spendable coins
    /// 
    /// # Arguments
    /// 
    /// * `inputs` the inputs which should be spent
    /// * `fund_value` value which should be transferred to a receiver (IN NANO GRIN)
    /// * `fee` transaction fee
    /// * `timelock` optional transaction timelock
    pub fn spend_coins(&mut self, inputs : Vec<MWCoin>, fund_value : u64, timelock : u32, num_of_outputs : usize) -> Result<SpendCoinsResult,String> {
        // Initial transaction slate
        let mut slate = Slate::blank(2, false);
        // Calculcate basefee based on number of inputs and expected outputs
        let fee = tx_fee(inputs.len(), num_of_outputs, 1);
        println!("Fee is {}", fee);

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
        if inputs.is_empty() {
            Err(String::from("No inputs provided"))
        }
        else if fund_value <= 0 {
            Err(String::from("Invalid parameters for fund_value or fee provided"))
        }
        else if inpval < (fund_value + fee) {
            Err(String::from("Spend coins function failed, input coins do not have enough value"))
        }
        else if duplicate {
            Err(String::from("Spend coins function failed, duplicate input coins provided"))
        }
        else {
            println!("Input coin value is {}", inpval);
            // Create needed blinding factors and nonce values
            let change_coin_key = create_secret_key(&mut self.rng, &self.secp);
            let sig_nonce = create_secret_key(&mut self.rng, &self.secp);
            let prf_nonce = create_secret_key(&mut self.rng, &self.secp);
            let rew_nonce = create_secret_key(&mut self.rng, &self.secp);
            let mut blind_sum = BlindSum::new();
            let offset = BlindingFactor::from_secret_key(create_secret_key(&mut self.rng, &self.secp));
            let out_bf = BlindingFactor::from_secret_key(change_coin_key.clone());
            blind_sum = blind_sum.add_blinding_factor(out_bf);

            slate.amount = fund_value;
            let feefield = FeeFields::new(0, fee).unwrap();
            slate.fee_fields = feefield;
            let mut tx = Transaction::empty();
    
            // Add the input coins
            let mut inp_vector : Vec<Input> = vec!();
            for coin in inputs {
                let commitment = deserialize_commitment(&coin.commitment);
                let inp_key = deserialize_secret_key(&coin.blinding_factor, &self.secp);
                let input = Input::new(OutputFeatures::Plain, commitment);
                inp_vector.push(input);
                let inp_bf = BlindingFactor::from_secret_key(inp_key.clone());
                blind_sum = blind_sum.sub_blinding_factor(inp_bf);
            }
            let inputs = Inputs::FeaturesAndCommit(inp_vector);
            tx = Transaction::new(inputs, &tx.body.outputs, tx.body.kernels());
            let final_bf = self.chain.blind_sum(&blind_sum)
                .expect("Failure when calcuclating blinding factor sum");
    
            // Add changecoin output
            let change_value = inpval - fund_value - fee;
            println!("Creating change coin with value {}", change_value);
            let commitment = self.secp.commit(change_value, change_coin_key.clone())
                .expect("Failed to genere pedersen commitment for change output coin");
            // Compute bulletproof rangeproof
            let proof = self.secp.bullet_proof(change_value, change_coin_key.clone(), rew_nonce, prf_nonce, None, None);
            let output = Output::new(OutputFeatures::Plain, commitment, proof);
            tx = tx.with_output(output);
            tx.offset = offset.clone();
            slate.tx = Some(tx);
            let final_key = final_bf.split(&offset, &self.secp)
                .unwrap()
                .secret_key(&self.secp)
                .unwrap();
            let mut ctx : Context = Context{
                parent_key_id: Identifier::zero(),
                sec_key: final_key.clone(),
                sec_nonce: sig_nonce.clone(),
                initial_sec_key: final_bf.secret_key(&self.secp)
                    .unwrap(),
                initial_sec_nonce: sig_nonce.clone(),
                output_ids: vec!(),
                input_ids: vec!(),
                amount: fund_value,
                fee: Some(feefield),
                payment_proof_derivation_index: None,
                late_lock_args: None,
                calculated_excess: None,
            };
            slate.fill_round_1(&self.chain, &mut ctx)
                .expect("Failed to complete round 1 on the senders turn");
    
            Ok(SpendCoinsResult {
                slate : slate,
                sig_key : final_key.clone(),
                sig_nonce : sig_nonce.clone(),
                change_coin : MWCoin::new(&commitment, &change_coin_key, change_value)
            })
        }
    }

    /// Implementation of the receive coins algorithm of the thesis
    /// Returns an updated pre-transaction (slate) with one partial signature added
    /// and a spendable output coins
    /// 
    /// # Arguments
    /// 
    /// * `slate` the pre-transaction slate as received from the sender
    /// * `fund_value` the value that should be transferred to the reciever
    pub fn recv_coins(&mut self, mut slate : Slate, fund_value : u64) -> Result<RecvCoinsResult, String> {
        // Validate output coin rangeproofs
        let mut tx = slate.tx.unwrap();
        for out in tx.outputs() {
            let prf = out.proof;
            let com = out.identifier.commit;
            self.secp.verify_bullet_proof(com, prf, None)
                .expect("Failed to verify outputcoin rangeproof");
        }

        // Create new output coins
        let out_coin_key = create_secret_key(&mut self.rng, &self.secp);
        let out_bf_com = out_coin_key.clone();
        let out_bf_prf = out_coin_key.clone();
        let rew_nonce = create_secret_key(&mut self.rng, &self.secp);
        let prf_nonce = create_secret_key(&mut self.rng, &self.secp);
        let sig_nonce = create_secret_key(&mut self.rng, &self.secp);
        
        println!("Creating output coin with value {}", fund_value);
        let commitment = self.secp.commit(fund_value, out_coin_key.clone())
            .expect("Failed to generate pedersen commitment for recv_coins output coin");
        let proof = self.secp.bullet_proof(fund_value, out_coin_key.clone(), rew_nonce, prf_nonce, None, None);
        let output = Output::new(OutputFeatures::Plain, commitment, proof);

        tx = tx.with_output(output);
        slate.tx = Some(tx);
        let mut ctx : Context = Context {
            parent_key_id: Identifier::zero(),
            sec_key: out_coin_key.clone(),
            sec_nonce: sig_nonce.clone(),
            initial_sec_key: out_coin_key.clone(),
            initial_sec_nonce: sig_nonce.clone(),
            output_ids: vec!(),
            input_ids: vec!(),
            amount: fund_value,
            fee: Some(slate.fee_fields.clone()),
            payment_proof_derivation_index: None,
            late_lock_args: None,
            calculated_excess: None
        };
        slate.fill_round_1(&self.chain, &mut ctx)
            .expect("Failed to complete round 1 on receivers turn");

        // Signs the transaction
        slate.fill_round_2(&self.chain, &ctx.sec_key, &ctx.sec_nonce)
            .expect("Failed to complete round 2 on receivers turn");
        
        Ok(RecvCoinsResult {
            slate : slate,
            output_coin : MWCoin::new(&commitment, &out_coin_key, fund_value)
        })
    }


    /// Implementation of the finTx algorithm outlined in the thesis
    /// Returns the final transaction slate which can be broadcast to a Grin node
    /// 
    /// # Arguments
    ///
    /// * `slate` the pre-transaction slate as provided to the sender by the receiver
    /// * `sec_key` the senders signing key
    /// * `sec_nonce` the senders signing nonce
    pub fn fin_tx(&mut self, mut slate : Slate, sec_key : &SecretKey, sec_nonce : &SecretKey) -> Result<Slate,String> {
        // First we verify output coin rangeproofs
        let mut valid = true;
        match slate.tx {
            Some(ref tx) => {
                for out in tx.outputs() {
                    let prf = out.proof;
                    let com = out.identifier.commit;
                    self.secp.verify_bullet_proof(com, prf, None)
                        .expect("Failed to verify outputcoin rangeproof");
                }
            }
            None => {
                valid = false;
            }
        };
        if valid {
            slate.fill_round_2(&self.chain, sec_key, sec_nonce)
                .expect("Failed to complete round 2 on senders turn");
            slate.finalize(&self.chain)
                .expect("Failed to finalize transaction");
            Ok(slate)
        }
        else {
            Err(String::from("Invalid transaction supplied to fin_tx call"))
        }
    }
    
}

#[cfg(test)]
mod test {
    use crate::{constants::NANO_GRIN, grin::{grin_routines::{*}, grin_types::MWCoin}};
    use crate::grin::grin_core::GrinCore;
    use grin_wallet_libwallet::Slate;
    use grin_core::global::{set_local_chain_type, ChainTypes};
    
    #[test]
    fn test_spend_coins() {
        // Should create a pre-transaction with transaction value 600
        let mut core = GrinCore::new();
        let fund_value = grin_to_nanogrin(2);
        let coin = MWCoin {
            commitment : String::from("086061571ea044365c81b5232c261866265024bd5c3506b5526d80df0c6c5845c8"),
            blinding_factor : String::from("1682c7950a19dfbc2f2c409ff9517cc72d2bf1ffa9ab1f83746e843461e1112c"),
            value : fund_value * 2
        };
        let result = core.spend_coins(vec!(coin), fund_value, 0, 2).unwrap();
        let ser = serde_json::to_string(&result.slate).unwrap();
        assert_eq!(50000 - 600 - 20, result.change_coin.value);
        assert_eq!(600, result.slate.amount);
        assert_eq!(false, result.slate.tx.unwrap().inputs().is_empty());
        let deser = Slate::deserialize_upgrade(&ser).unwrap();
        assert_eq!(result.slate.id, deser.id);
    }

    #[test]
    fn test_recv_coins() {
        // Should create an updated partially signed pre-transaction 
        let str_slate = r#"{"ver":"4:3","id":"bf1f5d2e-8ffb-4c9b-a658-7aed327c25b6","sta":"S1","amt":"600","fee":"20","sigs":[{"xs":"039ace71252c3ebe2d6790376c3ac6af9c414df48dcd00d8d548967d67433c3870","nonce":"02876939f1b92f9b2f856089f4451cee6f6f0cefe66ef2af1bf4dbf9c443a1f037"}],"coms":[{"c":"086061571ea044365c81b5232c261866265024bd5c3506b5526d80df0c6c5845c8"},{"c":"099af6fa073af384af31664b3cfcddee666636c0573b65222cedc9cc6e61b25b59","p":"a32ea9621c2a5ee8922f4f6f1aa22faabee160aa9d2e415d1cd2261e06303605f8c2f578532bd2b4824cdbcb710562ac241c78b5e3c691020a23e6d2846932f001768b0791548c1edde37f8ec3a89297cfd3f60c9057417c2b353e781c76c4a0c61d2d84d2135d8f3ca9b9d733eaaaf09730435f6c4e24a7bb0ee050c16f4f8443e69055f5b54f810526ec9a3789ae57f1a57174661ecce18fe34a266ffedf84afa4841d92a253eee4525ec85b137bc32dad217ecba5ab8c4cb9b62ea20977d9ea5baddc71f9ece661b3464bf01d72e9aa3778ea09d3a2296b47eb70dd73a2473b9ffd1d7c800a19a7f3d5a9a78607045845f57ae526f9ec8062ae71c7b45215ce14c5e9c208befb4d9372e0a888891d342b06246393ddb119332bc1d1b91e06cf07112883b5c67e34626579d54cf8af4b23912e3e95bcab1069479f98f3a5bc19b7a8fef32c1cd27cc89fa9290d85e92ac94ab423d66c1786fcd12ce98e50de76e3012f1e50851e8ab19185fdbc21c59ad7eb9bd4d99900e490e9c6f796216366d772dff3bfd10549b3bcad2c65106ea4b2a32bedce35d6e12a4ac2d1cd8d9fa409af5cc4fde46056648b25af161d793b9ff48bcdd1214b6062b0fec7f90bf38bdfb28844423ffe968afe2ed7397ad973bb87ba37533d824d09eacd82cc76b9c652d553eb4ac45a317dfb76c07619cd611943a06ee43222c103bd2067293da24604e53f779d5aa8f147104c302313b316e887399eac288ccf0721a68c7e3c7445fb076c02ee7b4094e964cd096d1bb366b4840adbc128d937f8e7f11df3a2cd11d28964f8c61dcb65dfc2843077cf152d62f2f2f0e5949802e32538e871ad8c3bdc20e65ed99578916ac7ae2adfb2a2f9723a2d88a07ddb8a4e86e9d39cdb36c4a19de0ba3e997ce0a7083a91ef9d945e0307796b76e3add7109136ace877b53fccbd"}]}"#;
        let slate = Slate::deserialize_upgrade(&str_slate).unwrap();
        let mut core = GrinCore::new();
        let slateid = slate.id;
        let result = core.recv_coins(slate, 600).unwrap();
        let ser = serde_json::to_string(&result.slate).unwrap();
        println!("{}", ser);
        assert_eq!(600, result.output_coin.value);
        assert_eq!(result.slate.id, slateid);
        assert_eq!(2, result.slate.tx.unwrap().outputs().len());
    }

    #[test]
    fn test_fin_tx() {
        let fund_value = grin_to_nanogrin(2);
        let mut core = GrinCore::new();

        // Create some valid input coin
        let input_val = fund_value * 2;
        let input_bf = create_secret_key(&mut core.rng, &core.secp);
        let commitment = core.secp.commit(fund_value, input_bf.clone()).unwrap();
        let coin = MWCoin {
            commitment : serialize_commitment(&commitment),
            blinding_factor : serialize_secret_key(&input_bf),
            value : input_val
        };

        set_local_chain_type(ChainTypes::AutomatedTesting);
        let result1 = core.spend_coins(vec!(coin), fund_value,  0, 2).unwrap();
        println!("sec_key: {} nonce: {}", serialize_secret_key(&result1.sig_key), serialize_secret_key(&result1.sig_nonce));
        println!("slate after spend coins : {}", serde_json::to_string(&result1.slate).unwrap());
        let mut result2 = core.recv_coins(result1.slate, fund_value).unwrap();
        println!("slate after recv coins : {}", serde_json::to_string(&result2.slate).unwrap());
        let sec_key = result1.sig_key;
        let sec_nonce = result1.sig_nonce;
        result2.slate.update_kernel().unwrap();
        let fin_slate = core.fin_tx(result2.slate, &sec_key, &sec_nonce).unwrap();
        let ser = serde_json::to_string(&fin_slate).unwrap();
        println!("final slate: {}", ser);
    }
}