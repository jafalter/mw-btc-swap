use crate::grin::grin_routines::*;
use crate::grin::grin_types::MWCoin;
use crate::util::get_os_rng;
use grin_core::core::transaction::FeeFields;
use grin_core::core::transaction::OutputFeatures;
use grin_core::core::{Input, Inputs, Output, Transaction};
use grin_core::libtx::tx_fee;
use grin_keychain::{BlindSum, BlindingFactor, ExtKeychain, Identifier, Keychain};
use grin_util::secp::key::{PublicKey, SecretKey};
use grin_util::secp::pedersen::Commitment;
use grin_util::secp::{ContextFlag, Secp256k1};
use grin_wallet_libwallet::Context;
use grin_wallet_libwallet::Slate;
use rand::rngs::OsRng;

pub struct GrinCore {
    rng: OsRng,
    secp: Secp256k1,
    chain: ExtKeychain,
}

pub struct SpendCoinsResult {
    slate: Slate,
    sig_key: SecretKey,
    sig_nonce: SecretKey,
    change_coin: Option<MWCoin>,
}

pub struct RecvCoinsResult {
    slate: Slate,
    output_coin: MWCoin,
}

impl GrinCore {
    pub fn new() -> GrinCore {
        let rng = get_os_rng();
        let secp = Secp256k1::with_caps(ContextFlag::Commit);
        let keychain = ExtKeychain::from_random_seed(true).unwrap();
        GrinCore {
            rng: rng,
            secp: secp,
            chain: keychain,
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
    pub fn spend_coins(
        &mut self,
        inputs: Vec<MWCoin>,
        fund_value: u64,
        timelock: u32,
        num_of_outputs: usize,
    ) -> Result<SpendCoinsResult, String> {
        // Initial transaction slate
        let mut slate = Slate::blank(2, false);
        // Calculcate basefee based on number of inputs and expected outputs
        let fee = tx_fee(inputs.len(), num_of_outputs, 1);
        println!("Fee is {}", fee);

        // Some input param validations
        let mut inpval: u64 = 0;
        let mut duplicate = false;
        for (i, coin) in inputs.iter().enumerate() {
            inpval = inpval + coin.value;
            for (j, cmp) in inputs.iter().enumerate() {
                if i != j && coin.commitment == cmp.commitment {
                    duplicate = true;
                }
            }
        }
        if inputs.is_empty() {
            Err(String::from("No inputs provided"))
        } else if fund_value <= 0 {
            Err(String::from("Invalid parameters for fund_value provided"))
        } else if inpval < (fund_value + fee) {
            Err(String::from(
                "Spend coins function failed, input coins do not have enough value",
            ))
        } else if duplicate {
            Err(String::from(
                "Spend coins function failed, duplicate input coins provided",
            ))
        } else {
            println!("Input coin value is {}", inpval);
            // Create needed blinding factors and nonce values
            let change_coin_key = create_secret_key(&mut self.rng, &self.secp);
            let sig_nonce = create_secret_key(&mut self.rng, &self.secp);
            let prf_nonce = create_secret_key(&mut self.rng, &self.secp);
            let rew_nonce = create_secret_key(&mut self.rng, &self.secp);
            let mut blind_sum = BlindSum::new();
            let offset =
                BlindingFactor::from_secret_key(create_secret_key(&mut self.rng, &self.secp));
            let out_bf = BlindingFactor::from_secret_key(change_coin_key.clone());
            blind_sum = blind_sum.add_blinding_factor(out_bf);

            let feefield = FeeFields::new(0, fee).unwrap();
            let mut tx = Transaction::empty();
            slate.fee_fields = feefield;
            slate.amount = fund_value;
            slate.offset = offset.clone();

            // Add the input coins
            let mut inp_vector: Vec<Input> = vec![];
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
            let final_bf = self
                .chain
                .blind_sum(&blind_sum)
                .expect("Failure when calculating blinding factor sum");

            // Add changecoin output
            let change_value = inpval - fund_value - fee;
            // Only create an output coin if there is actually a change value
            let mut com:Option<Commitment> = None;
            if change_value > 0 {
                println!("Creating change coin with value {}", change_value);
                let commitment = self
                    .secp
                    .commit(change_value, change_coin_key.clone())
                    .expect("Failed to create change coin commitment");
                // Compute bulletproof rangeproof
                let proof = self.secp.bullet_proof(
                    change_value,
                    change_coin_key.clone(),
                    rew_nonce,
                    prf_nonce,
                    None,
                    None,
                );
                let output = Output::new(OutputFeatures::Plain, commitment, proof);
                tx = tx.with_output(output);
                com = Some(commitment.clone());
            } else {
                ();
            }
            tx.offset = offset.clone();
            slate.tx = Some(tx);
            let final_key = final_bf
                .split(&offset, &self.secp)
                .unwrap()
                .secret_key(&self.secp)
                .unwrap();
            let mut ctx: Context = Context {
                parent_key_id: Identifier::zero(),
                sec_key: final_key.clone(),
                sec_nonce: sig_nonce.clone(),
                initial_sec_key: final_bf.secret_key(&self.secp).unwrap(),
                initial_sec_nonce: sig_nonce.clone(),
                output_ids: vec![],
                input_ids: vec![],
                amount: fund_value,
                fee: Some(feefield),
                payment_proof_derivation_index: None,
                late_lock_args: None,
                calculated_excess: None,
            };
            slate
                .fill_round_1(&self.chain, &mut ctx)
                .expect("Failed to complete round 1 on the senders turn");
            let change_coin_output = if com == None { None } else { Some(MWCoin::new(&com.unwrap(), &change_coin_key, change_value)) };

            Ok(SpendCoinsResult {
                slate: slate,
                sig_key: final_key.clone(),
                sig_nonce: sig_nonce.clone(),
                change_coin: change_coin_output,
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
    pub fn recv_coins(
        &mut self,
        mut slate: Slate,
        fund_value: u64,
    ) -> Result<RecvCoinsResult, String> {
        // Validate output coin rangeproofs
        let mut tx = slate.tx.unwrap();
        for out in tx.outputs() {
            let prf = out.proof;
            let com = out.identifier.commit;
            self.secp
                .verify_bullet_proof(com, prf, None)
                .expect("Failed to verify outputcoin rangeproof");
        }

        // Create new output coins
        let out_coin_key = create_secret_key(&mut self.rng, &self.secp);
        let rew_nonce = create_secret_key(&mut self.rng, &self.secp);
        let prf_nonce = create_secret_key(&mut self.rng, &self.secp);
        let sig_nonce = create_secret_key(&mut self.rng, &self.secp);

        println!("Creating output coin with value {}", fund_value);
        let commitment = self
            .secp
            .commit(fund_value, out_coin_key.clone())
            .expect("Failed to generate pedersen commitment for recv_coins output coin");
        let proof = self.secp.bullet_proof(
            fund_value,
            out_coin_key.clone(),
            rew_nonce,
            prf_nonce,
            None,
            None,
        );
        let output = Output::new(OutputFeatures::Plain, commitment, proof);

        tx = tx.with_output(output);
        slate.tx = Some(tx);
        slate
            .update_kernel()
            .expect("Failed to udpate kernel in recv_coins");
        let mut ctx: Context = Context {
            parent_key_id: Identifier::zero(),
            sec_key: out_coin_key.clone(),
            sec_nonce: sig_nonce.clone(),
            initial_sec_key: out_coin_key.clone(),
            initial_sec_nonce: sig_nonce.clone(),
            output_ids: vec![],
            input_ids: vec![],
            amount: fund_value,
            fee: Some(slate.fee_fields.clone()),
            payment_proof_derivation_index: None,
            late_lock_args: None,
            calculated_excess: None,
        };
        slate
            .fill_round_1(&self.chain, &mut ctx)
            .expect("Failed to complete round 1 on receivers turn");

        // Signs the transaction
        slate
            .fill_round_2(&self.chain, &ctx.sec_key, &ctx.sec_nonce)
            .expect("Failed to complete round 2 on receivers turn");

        Ok(RecvCoinsResult {
            slate: slate,
            output_coin: MWCoin::new(&commitment, &out_coin_key, fund_value),
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
    pub fn fin_tx(
        &mut self,
        mut slate: Slate,
        sec_key: &SecretKey,
        sec_nonce: &SecretKey,
    ) -> Result<Slate, String> {
        // First we verify output coin rangeproofs
        let mut valid = true;
        match slate.tx {
            Some(ref tx) => {
                for out in tx.outputs() {
                    let prf = out.proof;
                    let com = out.identifier.commit;
                    self.secp
                        .verify_bullet_proof(com, prf, None)
                        .expect("Failed to verify outputcoin rangeproof");
                }
            }
            None => {
                valid = false;
            }
        };
        if valid {
            slate
                .fill_round_2(&self.chain, sec_key, sec_nonce)
                .expect("Failed to complete round 2 on senders turn");
            slate
                .finalize(&self.chain)
                .expect("Failed to finalize transaction");
            Ok(slate)
        } else {
            Err(String::from("Invalid transaction supplied to fin_tx call"))
        }
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use crate::grin::grin_core::GrinCore;
    use crate::grin::{grin_routines::*, grin_types::MWCoin};
    use grin_core::{
        core::{verifier_cache::LruVerifierCache, Weighting},
        global::{set_local_chain_type, ChainTypes},
    };
    use grin_util::RwLock;
    use grin_wallet_libwallet::Slate;

    #[test]
    fn test_spend_coins() {
        set_local_chain_type(ChainTypes::AutomatedTesting);
        let mut core = GrinCore::new();
        let fund_value = grin_to_nanogrin(2);
        // Create some valid input coin
        let input_val = fund_value * 2;
        let input_bf = create_secret_key(&mut core.rng, &core.secp);
        let commitment = core.secp.commit(input_val, input_bf.clone()).unwrap();
        let coin = MWCoin {
            commitment: serialize_commitment(&commitment),
            blinding_factor: serialize_secret_key(&input_bf),
            value: input_val,
        };

        let result = core.spend_coins(vec![coin], fund_value, 0, 2).unwrap();
        let ser = serde_json::to_string(&result.slate).unwrap();
        let tx = result.slate.tx.unwrap();
        let fee: u64 = result.slate.fee_fields.fee(0);
        assert_eq!(input_val - fund_value - fee, result.change_coin.unwrap().value);
        assert_eq!(fund_value, result.slate.amount);
        assert_eq!(false, tx.inputs().is_empty());
        let deser = Slate::deserialize_upgrade(&ser).unwrap();
        assert_eq!(result.slate.id, deser.id);
    }

    #[test]
    #[should_panic(expected = "No inputs provided")]
    fn test_spend_coin_no_inputs() {
        let mut core = GrinCore::new();
        let fund_value = grin_to_nanogrin(2);
        core.spend_coins(vec![], fund_value, 0, 2).unwrap();
    }

    #[test]
    #[should_panic(expected = "Invalid parameters for fund_value provided")]
    fn test_spend_coins_invalid_fundvalue() {
        let mut core = GrinCore::new();
        let fund_value = 0;
        // Create some valid input coin
        let input_val = fund_value * 2;
        let input_bf = create_secret_key(&mut core.rng, &core.secp);
        let commitment = core.secp.commit(input_val, input_bf.clone()).unwrap();
        let coin = MWCoin {
            commitment: serialize_commitment(&commitment),
            blinding_factor: serialize_secret_key(&input_bf),
            value: input_val,
        };
        core.spend_coins(vec![coin], fund_value, 0, 2).unwrap();
    }

    #[test]
    #[should_panic(expected = "Spend coins function failed, input coins do not have enough value")]
    fn test_spend_coins_too_little_input_funds() {
        let mut core = GrinCore::new();
        let fund_value = grin_to_nanogrin(1);
        // Create some valid input coin
        let input_val = fund_value - 1;
        let input_bf = create_secret_key(&mut core.rng, &core.secp);
        let commitment = core.secp.commit(input_val, input_bf.clone()).unwrap();
        let coin = MWCoin {
            commitment: serialize_commitment(&commitment),
            blinding_factor: serialize_secret_key(&input_bf),
            value: input_val,
        };
        core.spend_coins(vec![coin], fund_value, 0, 2).unwrap();
    }

    #[test]
    #[should_panic(expected = "Spend coins function failed, duplicate input coins provided")]
    fn test_spend_coins_duplicate_inputs() {
        let mut core = GrinCore::new();
        let fund_value = grin_to_nanogrin(1);
        // Create some valid input coin
        let input_val = fund_value * 2;
        let input_bf = create_secret_key(&mut core.rng, &core.secp);
        let commitment = core.secp.commit(input_val, input_bf.clone()).unwrap();
        let coin = MWCoin {
            commitment: serialize_commitment(&commitment),
            blinding_factor: serialize_secret_key(&input_bf),
            value: input_val,
        };
        let coin2 = MWCoin {
            commitment: serialize_commitment(&commitment),
            blinding_factor: serialize_secret_key(&input_bf),
            value: input_val,
        };
        core.spend_coins(vec![coin, coin2], fund_value, 0, 2).unwrap();
    }

    #[test]
    fn test_recv_coins() {
        // Should create an updated partially signed pre-transaction
        let str_slate = r#"{"ver":"4:3","id":"0ef39863-c759-44de-892c-538826e3a8f8","sta":"S1","off":"d3c5484ee792e95b9c83583154a4b6c9df31cb3b3c46d080b35841e809e9d02d","amt":"2000000000","fee":"23000000","sigs":[{"xs":"02a4c554bebf29b4361a582dfcb689cf08472673d2c71df8416ed3d4352a4f5f4e","nonce":"034bdb3f5d6dd8a08edaf86722cf14214908c527725f7a0298428cb82e76724dbf"}],"coms":[{"c":"08a6d28ddfc43a95b391cac473d6778d29f973f0b2886b4768aed393502936d82b"},{"c":"081046ca3d0fa2d298c855de4a7454fd5e537fd21674c3b6d3f82bc5884c54b5a7","p":"05c7e1be11bd3358cbad8931a176a099a220ff553eb8eea8b43da0297702486af5260b404f0f9fa66e808a73a1518bf3af3751c73e162a7a80fd7f57574c176e0486bfcd4055e7f9dd8ab2ceacc2baeae25f78bce79058338f38cc0e5b8624f599056153a14250e134bfd78e95a8cffa718c71dbe3fa39d5fa0c177be9258f96832e12cc7e37d0fde54d1012cf3a64c1ab913ebfdeb0790a6e4b78eaff7db9f205fd57f9603f7736a6babd37036ed47f69a472c9ee9ece15c1bb32fdbabfaf3afe148cb16e4fcf6d6ba1945b5dc3e488ad28745f0283468efe901fb8f4c328b178d532aa99fcb3132f8d0d4bc2a91a106ff97395c2fc6414799f06bd839de8883d9fcca6a4f62fe08ac9350283db0590614264458626e05549bf2ffd1ffe4ab0a526f9677afb0d92efa452d760145e5a72142d19cb5715ddfcb061579c588192a1183dad37eeea538726a9f253a2ef7687a9b5b600154f04f51766403a03d7a4aa1703ed63dc67df48b3addcbc3bd7285aebc6b153b747992f82f43aabb0246f04f3d3ae355c91860c61f464a46cf32d68ade9f8cb9b60eb86a8915a86c426ff002552c4ce179ccdbceaa9005d706dc735157091b1af914ea1c69e4eee7aeaeabfeb028b17ef345ca8dc325fe8d7e82cdf19eeb1d5153a1dc03ddd343685cce6d915d71a24ddfbe156cce1c3630513aa426c693c0f5e6d290511e6b37b66a3d7ad2e22ddf0656c3a56c7a48edcac51cd55ff913aa311e9a1057573fae3e7b3c91ccc52813741cdec72bb3be1ff592cbdc42511ddda390dea7e9fd5fdd38c2d13e7dc6aabd12ac67d8e6ea2625c0a0444f9215113627f637434febb4f364c3e7ef9dbe202e9540f5a42d7aa30db39e4f96074491d6294bfa941fd150d08336a6a6aad1e057da6363ecb11313532e1ea5328283148a5dfca277ae516e09f69c17344eb41"}]}"#;
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
    #[should_panic(expected = "Failed to verify outputcoin rangeproof: InvalidRangeProof")]
    fn test_recv_coins_invalid_proof() {
        // Should create an updated partially signed pre-transaction
        let str_slate = r#"{"ver":"4:3","id":"0ef39863-c759-44de-892c-538826e3a8f8","sta":"S1","off":"d3c5484ee792e95b9c83583154a4b6c9df31cb3b3c46d080b35841e809e9d02d","amt":"2000000000","fee":"23000000","sigs":[{"xs":"02a4c554bebf29b4361a582dfcb689cf08472673d2c71df8416ed3d4352a4f5f4e","nonce":"034bdb3f5d6dd8a08edaf86722cf14214908c527725f7a0298428cb82e76724dbf"}],"coms":[{"c":"08a6d28ddfc43a95b391cac473d6778d29f973f0b2886b4768aed393502936d82b"},{"c":"081046ca3d0fa2d298c855de4a7454fd5e537fd21674c3b6d3f82bc5884c54b5a7","p":"05c7e1be11bd3358cbad8931a176a099a220ff553eb8eea8b43da0297702486af5260b404f0f9fa66e808a73a1518bf3af3751c73e162a7a80fd7f57574c176e0486bfcd4055e7f9dd8ab2ceacc2baeae25f78bce79058338f38cc0e5b8624f599056153a14250e134bfd78e95a8cffa718c71dbe3fa39d5fa0c177be9258f96832e12cc7e37d0fde54d1012cf3a64c1ab913ebfdeb0790a6e4b78eaff7db9f205fd57f9603f7736a6babd37036ed47f69a472c9ee9ece15c1bb32fdbabfaf3afe148cb16e4fcf6d6ba1945b5dc3e488ad28745f0283468efe901fb8f4c328b178d532aa99fcb3132f8d0d4bc2a91a106ff97395c2fc6414799f06bd839de8883d9fcca6a4f62fe08ac9350283db0590614264458626e05549bf2ffd1ffe4ab0a526f9677afb0d92efa452d760145e5a72142d19cb5715ddfcb061579c588192a1183dad37eeea538726a9f253a2ef7687a9b5b600154f04f51766403a03d7a4aa1703ed63dc67df48b3addcbc3bd7285aebc6b153b747992f82f43aabb0246f04f3d3ae355c91860c61f464a46cf32d68ade9f8cb9b60eb86a8915a86c426ff002552c4ce179ccdbceaa9005d706dc735157091b1af914ea1c69e4eee7aeaeabfeb028b17ef345ca8dc325fe8d7e82cdf19eeb1d5153a1dc03ddd343685cce6d915d71a24ddfbe156cce1c3630513aa426c693c0f5e6d290511e6b37b66a3d7ad2e22ddf0656c3a56c7a48edcac51cd55ff913aa311e9a1057573fae3e7b3c91ccc52813741cdec72bb3be1ff592cbdc42511ddda390dea7e9fd5fdd38c2d13e7dc6aabd12ac67d8e6ea2625c0a0444f9215113627f637434febb4f364c3e7ef9dbe202e9540f5a42d7aa30db39e4f96074491d6294bfa941fd150d08336a6a6aad1e057da6363ecb11313532e1ea5328283148a5dfca277ae516e09f69c17344eb42"}]}"#;
        let slate = Slate::deserialize_upgrade(&str_slate).unwrap();
        let mut core = GrinCore::new();
        core.recv_coins(slate, 600).unwrap();
    }

    #[test]
    fn test_fin_tx() {
        set_local_chain_type(ChainTypes::AutomatedTesting);
        let mut core = GrinCore::new();
        let str_slate = r#"{"ver":"4:3","id":"0ef39863-c759-44de-892c-538826e3a8f8","sta":"S1","off":"d3c5484ee792e95b9c83583154a4b6c9df31cb3b3c46d080b35841e809e9d02d","amt":"2000000000","fee":"23000000","sigs":[{"xs":"02a4c554bebf29b4361a582dfcb689cf08472673d2c71df8416ed3d4352a4f5f4e","nonce":"034bdb3f5d6dd8a08edaf86722cf14214908c527725f7a0298428cb82e76724dbf"},{"xs":"022d6700b009ac43b1ce52bc487728744849ce772f2fc305a8c474fa3a62870e89","nonce":"02933e2e529efe010269a0d8bb2925ba127b66deb7bae25a94aaa0a8d665e61db4","part":"b41de665d6a8a0aa945ae2bab7de667b12ba2529bbd8a0690201fe9e522e3e933bcadbfff1e9b27b6a404f8cfe94d338b9c68618ed56fa81a93bb27de79a7ed8"}],"coms":[{"c":"08a6d28ddfc43a95b391cac473d6778d29f973f0b2886b4768aed393502936d82b"},{"c":"08e0534f3747d17221dc19e285694eb018af14727896c219ee5663e9d7c683fbec","p":"5effa9d9e66e983a8f48376113984eb18796c9ac9ac19097b6f3c5012b5d00acfe22d4a5f65357ca8e3decfaaf86cb4cefb6b7185db92643692f872eec8fb5b3074f07ead20fe14c8aec48c22aedc5459b77d9591af672e97bac752f5137f47c1799952fdc5a8ae361dccece1cc9c838e9817a0c8b9691d2cadc83c436acfffdffed7455f45383bc31a19fa90b8d2d32131dbedb25736f3fd7c1e00cb5483c68a121f8f53a2e6a0b54aa6a7adcfc1e2ca20b85be0b9663535a75bbf00e32f0c0e6cdcf5f28c70e3d1cddf9fbf35c63f9206d95d347c7cc2576ccda5832bfac7bc40fa86eb1f9a2a32089d4c2f542d8129ea4360e2e1a6ea02057aa244b4401bf0ec86732bba4c8be97c2e9fb14eb4eaeba237d9d2f35070430381f1cbc59190f7e58c5b594de7cf4af4428ef024874a569bed4eb933d5ec830937e7f5de31dfd9b103a94b878798b5a6fc9093c16f81366fa67ecf9ee9b2455a4a5e8d51d1d02d25001d7e1da14f56c7fd3d8df60aa87af19ef25a549f1f3450e5c444dc8010c8d5db63f9db66720112eca236186ab4ed45f9ab9da740b7e2233b87b473458a90fb0a1b35295753c003e39d79648add3542473e5e7dce0c1c0fc7bd0d8d039ec9fc80b61aabdfbfb829234ad73d365daefc3cdd4a4014dc4170f0254be7e5b0432e0900e8d3ef41e8a3c97702d8354e781110b8bd9e85258af1d55f2a1697073be44b0cdc774b93848d245e6702b883ec9a1b3a504838bba08f059c14d89f54a63af84e57cfe19ae634fb84c921fe892923443b6bb9766837313c62282633afd3674066bfc5f10578638800277e43426ee953f6fd419656a7fc8eeb857f8f70e0eccb84ac0596915404087885a3a254362f08a308dd14ab9b2377c28505bc9c5e7daae960c190e455b8c85d68070aadb4b2dfb90ab3a8f886cee527c53be9e2665962c"},{"c":"081046ca3d0fa2d298c855de4a7454fd5e537fd21674c3b6d3f82bc5884c54b5a7","p":"05c7e1be11bd3358cbad8931a176a099a220ff553eb8eea8b43da0297702486af5260b404f0f9fa66e808a73a1518bf3af3751c73e162a7a80fd7f57574c176e0486bfcd4055e7f9dd8ab2ceacc2baeae25f78bce79058338f38cc0e5b8624f599056153a14250e134bfd78e95a8cffa718c71dbe3fa39d5fa0c177be9258f96832e12cc7e37d0fde54d1012cf3a64c1ab913ebfdeb0790a6e4b78eaff7db9f205fd57f9603f7736a6babd37036ed47f69a472c9ee9ece15c1bb32fdbabfaf3afe148cb16e4fcf6d6ba1945b5dc3e488ad28745f0283468efe901fb8f4c328b178d532aa99fcb3132f8d0d4bc2a91a106ff97395c2fc6414799f06bd839de8883d9fcca6a4f62fe08ac9350283db0590614264458626e05549bf2ffd1ffe4ab0a526f9677afb0d92efa452d760145e5a72142d19cb5715ddfcb061579c588192a1183dad37eeea538726a9f253a2ef7687a9b5b600154f04f51766403a03d7a4aa1703ed63dc67df48b3addcbc3bd7285aebc6b153b747992f82f43aabb0246f04f3d3ae355c91860c61f464a46cf32d68ade9f8cb9b60eb86a8915a86c426ff002552c4ce179ccdbceaa9005d706dc735157091b1af914ea1c69e4eee7aeaeabfeb028b17ef345ca8dc325fe8d7e82cdf19eeb1d5153a1dc03ddd343685cce6d915d71a24ddfbe156cce1c3630513aa426c693c0f5e6d290511e6b37b66a3d7ad2e22ddf0656c3a56c7a48edcac51cd55ff913aa311e9a1057573fae3e7b3c91ccc52813741cdec72bb3be1ff592cbdc42511ddda390dea7e9fd5fdd38c2d13e7dc6aabd12ac67d8e6ea2625c0a0444f9215113627f637434febb4f364c3e7ef9dbe202e9540f5a42d7aa30db39e4f96074491d6294bfa941fd150d08336a6a6aad1e057da6363ecb11313532e1ea5328283148a5dfca277ae516e09f69c17344eb41"}]}"#;
        let sk = deserialize_secret_key(
            &String::from("4f9851e6252daec8a0cec6e16ee16184e0da5024f5cc3dae49096bc778483594"),
            &core.secp,
        );
        let nonce = deserialize_secret_key(
            &String::from("01c41476c59be2bdf5f88e2c43aa5b2133c6b38f241754b19c167912e6df2fb3"),
            &core.secp,
        );
        let slate = Slate::deserialize_upgrade(&str_slate).unwrap();
        let fin_slate = core.fin_tx(slate, &sk, &nonce).unwrap();
        let ser = serde_json::to_string(&fin_slate).unwrap();
        println!("final slate: {}", ser);
        let verifier_cache = Arc::new(RwLock::new(LruVerifierCache::new()));
        assert_eq!(
            Ok(()),
            fin_slate
                .tx
                .unwrap()
                .validate(Weighting::AsTransaction, verifier_cache, 0)
        );
    }

    #[test]
    #[should_panic(expected = "Failed to verify outputcoin rangeproof: InvalidRangeProof")]
    fn test_fin_tx_invalid_rproof() {
        set_local_chain_type(ChainTypes::AutomatedTesting);
        let mut core = GrinCore::new();
        let str_slate = r#"{"ver":"4:3","id":"0ef39863-c759-44de-892c-538826e3a8f8","sta":"S1","off":"d3c5484ee792e95b9c83583154a4b6c9df31cb3b3c46d080b35841e809e9d02d","amt":"2000000000","fee":"23000000","sigs":[{"xs":"02a4c554bebf29b4361a582dfcb689cf08472673d2c71df8416ed3d4352a4f5f4e","nonce":"034bdb3f5d6dd8a08edaf86722cf14214908c527725f7a0298428cb82e76724dbf"},{"xs":"022d6700b009ac43b1ce52bc487728744849ce772f2fc305a8c474fa3a62870e89","nonce":"02933e2e529efe010269a0d8bb2925ba127b66deb7bae25a94aaa0a8d665e61db4","part":"b41de665d6a8a0aa945ae2bab7de667b12ba2529bbd8a0690201fe9e522e3e933bcadbfff1e9b27b6a404f8cfe94d338b9c68618ed56fa81a93bb27de79a7ed8"}],"coms":[{"c":"08a6d28ddfc43a95b391cac473d6778d29f973f0b2886b4768aed393502936d82b"},{"c":"08e0534f3747d17221dc19e285694eb018af14727896c219ee5663e9d7c683fbec","p":"5effa9d9e66e983a8f48376113984eb18796c9ac9ac19097b6f3c5012b5d00acfe22d4a5f65357ca8e3decfaaf86cb4cefb6b7185db92643692f872eec8fb5b3074f07ead20fe14c8aec48c22aedc5459b77d9591af672e97bac752f5137f47c1799952fdc5a8ae361dccece1cc9c838e9817a0c8b9691d2cadc83c436acfffdffed7455f45383bc31a19fa90b8d2d32131dbedb25736f3fd7c1e00cb5483c68a121f8f53a2e6a0b54aa6a7adcfc1e2ca20b85be0b9663535a75bbf00e32f0c0e6cdcf5f28c70e3d1cddf9fbf35c63f9206d95d347c7cc2576ccda5832bfac7bc40fa86eb1f9a2a32089d4c2f542d8129ea4360e2e1a6ea02057aa244b4401bf0ec86732bba4c8be97c2e9fb14eb4eaeba237d9d2f35070430381f1cbc59190f7e58c5b594de7cf4af4428ef024874a569bed4eb933d5ec830937e7f5de31dfd9b103a94b878798b5a6fc9093c16f81366fa67ecf9ee9b2455a4a5e8d51d1d02d25001d7e1da14f56c7fd3d8df60aa87af19ef25a549f1f3450e5c444dc8010c8d5db63f9db66720112eca236186ab4ed45f9ab9da740b7e2233b87b473458a90fb0a1b35295753c003e39d79648add3542473e5e7dce0c1c0fc7bd0d8d039ec9fc80b61aabdfbfb829234ad73d365daefc3cdd4a4014dc4170f0254be7e5b0432e0900e8d3ef41e8a3c97702d8354e781110b8bd9e85258af1d55f2a1697073be44b0cdc774b93848d245e6702b883ec9a1b3a504838bba08f059c14d89f54a63af84e57cfe19ae634fb84c921fe892923443b6bb9766837313c62282633afd3674066bfc5f10578638800277e43426ee953f6fd419656a7fc8eeb857f8f70e0eccb84ac0596915404087885a3a254362f08a308dd14ab9b2377c28505bc9c5e7daae960c190e455b8c85d68070aadb4b2dfb90ab3a8f886cee527c53be9e26659622"},{"c":"081046ca3d0fa2d298c855de4a7454fd5e537fd21674c3b6d3f82bc5884c54b5a7","p":"05c7e1be11bd3358cbad8931a176a099a220ff553eb8eea8b43da0297702486af5260b404f0f9fa66e808a73a1518bf3af3751c73e162a7a80fd7f57574c176e0486bfcd4055e7f9dd8ab2ceacc2baeae25f78bce79058338f38cc0e5b8624f599056153a14250e134bfd78e95a8cffa718c71dbe3fa39d5fa0c177be9258f96832e12cc7e37d0fde54d1012cf3a64c1ab913ebfdeb0790a6e4b78eaff7db9f205fd57f9603f7736a6babd37036ed47f69a472c9ee9ece15c1bb32fdbabfaf3afe148cb16e4fcf6d6ba1945b5dc3e488ad28745f0283468efe901fb8f4c328b178d532aa99fcb3132f8d0d4bc2a91a106ff97395c2fc6414799f06bd839de8883d9fcca6a4f62fe08ac9350283db0590614264458626e05549bf2ffd1ffe4ab0a526f9677afb0d92efa452d760145e5a72142d19cb5715ddfcb061579c588192a1183dad37eeea538726a9f253a2ef7687a9b5b600154f04f51766403a03d7a4aa1703ed63dc67df48b3addcbc3bd7285aebc6b153b747992f82f43aabb0246f04f3d3ae355c91860c61f464a46cf32d68ade9f8cb9b60eb86a8915a86c426ff002552c4ce179ccdbceaa9005d706dc735157091b1af914ea1c69e4eee7aeaeabfeb028b17ef345ca8dc325fe8d7e82cdf19eeb1d5153a1dc03ddd343685cce6d915d71a24ddfbe156cce1c3630513aa426c693c0f5e6d290511e6b37b66a3d7ad2e22ddf0656c3a56c7a48edcac51cd55ff913aa311e9a1057573fae3e7b3c91ccc52813741cdec72bb3be1ff592cbdc42511ddda390dea7e9fd5fdd38c2d13e7dc6aabd12ac67d8e6ea2625c0a0444f9215113627f637434febb4f364c3e7ef9dbe202e9540f5a42d7aa30db39e4f96074491d6294bfa941fd150d08336a6a6aad1e057da6363ecb11313532e1ea5328283148a5dfca277ae516e09f69c17344eb41"}]}"#;
        let sk = deserialize_secret_key(
            &String::from("4f9851e6252daec8a0cec6e16ee16184e0da5024f5cc3dae49096bc778483594"),
            &core.secp,
        );
        let nonce = deserialize_secret_key(
            &String::from("01c41476c59be2bdf5f88e2c43aa5b2133c6b38f241754b19c167912e6df2fb3"),
            &core.secp,
        );
        let slate = Slate::deserialize_upgrade(&str_slate).unwrap();
        core.fin_tx(slate, &sk, &nonce).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_fin_tx_invalid_secret() {
        set_local_chain_type(ChainTypes::AutomatedTesting);
        let mut core = GrinCore::new();
        let str_slate = r#"{"ver":"4:3","id":"0ef39863-c759-44de-892c-538826e3a8f8","sta":"S1","off":"d3c5484ee792e95b9c83583154a4b6c9df31cb3b3c46d080b35841e809e9d02d","amt":"2000000000","fee":"23000000","sigs":[{"xs":"02a4c554bebf29b4361a582dfcb689cf08472673d2c71df8416ed3d4352a4f5f4e","nonce":"034bdb3f5d6dd8a08edaf86722cf14214908c527725f7a0298428cb82e76724dbf"},{"xs":"022d6700b009ac43b1ce52bc487728744849ce772f2fc305a8c474fa3a62870e89","nonce":"02933e2e529efe010269a0d8bb2925ba127b66deb7bae25a94aaa0a8d665e61db4","part":"b41de665d6a8a0aa945ae2bab7de667b12ba2529bbd8a0690201fe9e522e3e933bcadbfff1e9b27b6a404f8cfe94d338b9c68618ed56fa81a93bb27de79a7ed8"}],"coms":[{"c":"08a6d28ddfc43a95b391cac473d6778d29f973f0b2886b4768aed393502936d82b"},{"c":"08e0534f3747d17221dc19e285694eb018af14727896c219ee5663e9d7c683fbec","p":"5effa9d9e66e983a8f48376113984eb18796c9ac9ac19097b6f3c5012b5d00acfe22d4a5f65357ca8e3decfaaf86cb4cefb6b7185db92643692f872eec8fb5b3074f07ead20fe14c8aec48c22aedc5459b77d9591af672e97bac752f5137f47c1799952fdc5a8ae361dccece1cc9c838e9817a0c8b9691d2cadc83c436acfffdffed7455f45383bc31a19fa90b8d2d32131dbedb25736f3fd7c1e00cb5483c68a121f8f53a2e6a0b54aa6a7adcfc1e2ca20b85be0b9663535a75bbf00e32f0c0e6cdcf5f28c70e3d1cddf9fbf35c63f9206d95d347c7cc2576ccda5832bfac7bc40fa86eb1f9a2a32089d4c2f542d8129ea4360e2e1a6ea02057aa244b4401bf0ec86732bba4c8be97c2e9fb14eb4eaeba237d9d2f35070430381f1cbc59190f7e58c5b594de7cf4af4428ef024874a569bed4eb933d5ec830937e7f5de31dfd9b103a94b878798b5a6fc9093c16f81366fa67ecf9ee9b2455a4a5e8d51d1d02d25001d7e1da14f56c7fd3d8df60aa87af19ef25a549f1f3450e5c444dc8010c8d5db63f9db66720112eca236186ab4ed45f9ab9da740b7e2233b87b473458a90fb0a1b35295753c003e39d79648add3542473e5e7dce0c1c0fc7bd0d8d039ec9fc80b61aabdfbfb829234ad73d365daefc3cdd4a4014dc4170f0254be7e5b0432e0900e8d3ef41e8a3c97702d8354e781110b8bd9e85258af1d55f2a1697073be44b0cdc774b93848d245e6702b883ec9a1b3a504838bba08f059c14d89f54a63af84e57cfe19ae634fb84c921fe892923443b6bb9766837313c62282633afd3674066bfc5f10578638800277e43426ee953f6fd419656a7fc8eeb857f8f70e0eccb84ac0596915404087885a3a254362f08a308dd14ab9b2377c28505bc9c5e7daae960c190e455b8c85d68070aadb4b2dfb90ab3a8f886cee527c53be9e2665962c"},{"c":"081046ca3d0fa2d298c855de4a7454fd5e537fd21674c3b6d3f82bc5884c54b5a7","p":"05c7e1be11bd3358cbad8931a176a099a220ff553eb8eea8b43da0297702486af5260b404f0f9fa66e808a73a1518bf3af3751c73e162a7a80fd7f57574c176e0486bfcd4055e7f9dd8ab2ceacc2baeae25f78bce79058338f38cc0e5b8624f599056153a14250e134bfd78e95a8cffa718c71dbe3fa39d5fa0c177be9258f96832e12cc7e37d0fde54d1012cf3a64c1ab913ebfdeb0790a6e4b78eaff7db9f205fd57f9603f7736a6babd37036ed47f69a472c9ee9ece15c1bb32fdbabfaf3afe148cb16e4fcf6d6ba1945b5dc3e488ad28745f0283468efe901fb8f4c328b178d532aa99fcb3132f8d0d4bc2a91a106ff97395c2fc6414799f06bd839de8883d9fcca6a4f62fe08ac9350283db0590614264458626e05549bf2ffd1ffe4ab0a526f9677afb0d92efa452d760145e5a72142d19cb5715ddfcb061579c588192a1183dad37eeea538726a9f253a2ef7687a9b5b600154f04f51766403a03d7a4aa1703ed63dc67df48b3addcbc3bd7285aebc6b153b747992f82f43aabb0246f04f3d3ae355c91860c61f464a46cf32d68ade9f8cb9b60eb86a8915a86c426ff002552c4ce179ccdbceaa9005d706dc735157091b1af914ea1c69e4eee7aeaeabfeb028b17ef345ca8dc325fe8d7e82cdf19eeb1d5153a1dc03ddd343685cce6d915d71a24ddfbe156cce1c3630513aa426c693c0f5e6d290511e6b37b66a3d7ad2e22ddf0656c3a56c7a48edcac51cd55ff913aa311e9a1057573fae3e7b3c91ccc52813741cdec72bb3be1ff592cbdc42511ddda390dea7e9fd5fdd38c2d13e7dc6aabd12ac67d8e6ea2625c0a0444f9215113627f637434febb4f364c3e7ef9dbe202e9540f5a42d7aa30db39e4f96074491d6294bfa941fd150d08336a6a6aad1e057da6363ecb11313532e1ea5328283148a5dfca277ae516e09f69c17344eb41"}]}"#;
        let sk = deserialize_secret_key(
            &String::from("4f9851e6252daec8a0cec6e16ee16184e0da5024f5cc3dae49096bc778483593"),
            &core.secp,
        );
        let nonce = deserialize_secret_key(
            &String::from("01c41476c59be2bdf5f88e2c43aa5b2133c6b38f241754b19c167912e6df2fb3"),
            &core.secp,
        );
        let slate = Slate::deserialize_upgrade(&str_slate).unwrap();
        core.fin_tx(slate, &sk, &nonce).unwrap();
    }

    #[test]
    fn test_full_tx_flow() {
        let fund_value = grin_to_nanogrin(2);
        let mut core = GrinCore::new();

        // Create some valid input coin
        let input_val = fund_value * 2;
        let input_bf = create_secret_key(&mut core.rng, &core.secp);
        let commitment = core.secp.commit(input_val, input_bf.clone()).unwrap();
        let coin = MWCoin {
            commitment: serialize_commitment(&commitment),
            blinding_factor: serialize_secret_key(&input_bf),
            value: input_val,
        };

        set_local_chain_type(ChainTypes::AutomatedTesting);
        let result1 = core.spend_coins(vec![coin], fund_value, 0, 2).unwrap();
        println!(
            "sec_key: {} nonce: {}",
            serialize_secret_key(&result1.sig_key),
            serialize_secret_key(&result1.sig_nonce)
        );
        println!(
            "slate after spend coins : {}",
            serde_json::to_string(&result1.slate).unwrap()
        );
        let result2 = core.recv_coins(result1.slate, fund_value).unwrap();
        println!(
            "slate after recv coins : {}",
            serde_json::to_string(&result2.slate).unwrap()
        );
        let sec_key = result1.sig_key;
        let sec_nonce = result1.sig_nonce;
        let fin_slate = core.fin_tx(result2.slate, &sec_key, &sec_nonce).unwrap();
        let ser = serde_json::to_string(&fin_slate).unwrap();
        println!("final slate: {}", ser);
    }
}