use crate::grin::grin_routines::*;
use crate::grin::grin_types::MWCoin;
use crate::util::get_os_rng;
use grin_core::core::transaction::OutputFeatures;
use grin_core::core::{transaction::FeeFields, KernelFeatures};
use grin_core::core::{Input, Inputs, Output, Transaction};
use grin_core::libtx::tx_fee;
use grin_keychain::{BlindSum, BlindingFactor, ExtKeychain, Identifier, Keychain};
use grin_util::secp::pedersen::Commitment;
use grin_util::secp::{
    aggsig,
    key::{PublicKey, SecretKey},
    Signature,
};
use grin_util::secp::{ContextFlag, Secp256k1};
use grin_wallet_libwallet::{
    slate_versions::v4::{KernelFeaturesArgsV4, SlateV4},
    Context, Slate,
};
use rand::rngs::OsRng;

pub struct GrinCore {
    pub rng: OsRng,
    pub secp: Secp256k1,
    pub chain: ExtKeychain,
}

pub struct SpendCoinsResult {
    pub slate: Slate,
    pub sig_key: SecretKey,
    pub sig_nonce: SecretKey,
    pub change_coin: Option<MWCoin>,
}

pub struct RecvCoinsResult {
    pub slate: Slate,
    pub output_coin: MWCoin,
}

pub struct AptRecCoinsResult {
    pub slate: Slate,
    pub output_coin: MWCoin,
    pub prt_sig: (Signature, Signature),
}

pub struct DRecvCoinsResult {
    pub slate: Slate,
    pub out_key_blind: SecretKey,
    pub sig_nonce: SecretKey,
    pub prf_nonce: SecretKey,
    pub prf_ctx: MPBPContext,
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
    /// * `num_participants` number of participants in the usual case this would be 2, if we intend to create or spend a multiouput then it should be 3 or 4
    pub fn spend_coins(
        &mut self,
        inputs: Vec<MWCoin>,
        fund_value: u64,
        timelock: u64,
        num_of_outputs: usize,
        num_participants: u8,
    ) -> Result<SpendCoinsResult, String> {
        // Initial transaction slate
        let lock_height_param = if timelock != 0 { Some(timelock) } else { None };
        let kernel_feat_param = if lock_height_param.is_some() { 2 } else { 0 };
        let mut slate = Slate::blank_with_kernel_features(
            num_participants,
            false,
            kernel_feat_param,
            lock_height_param,
        );
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

            let fee_field = FeeFields::new(0, fee).unwrap();
            let mut tx = Transaction::empty();
            slate.fee_fields = fee_field;
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
            let mut com: Option<Commitment> = None;
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
                fee: Some(fee_field),
                payment_proof_derivation_index: None,
                late_lock_args: None,
                calculated_excess: None,
            };
            slate
                .fill_round_1(&self.chain, &mut ctx)
                .expect("Failed to complete round 1 on the senders turn");
            let change_coin_output = if com == None {
                None
            } else {
                Some(MWCoin::new(&com.unwrap(), &change_coin_key, change_value))
            };

            Ok(SpendCoinsResult {
                slate: slate,
                sig_key: final_key.clone(),
                sig_nonce: sig_nonce.clone(),
                change_coin: change_coin_output,
            })
        }
    }

    /// Implementation of the dspendcoins algorithm as outlined in the thesis
    /// To run the full protocol the first sender would have to call the regular spend coins function
    /// The second one would then have to call this one
    ///
    /// # Arguments
    ///
    /// * `inputs` the input coins containing the shares of the blinding factor
    /// * `slate` Transaction slate as provided by the first sender
    /// * `fund_value` amount that should be spend and transferred to a receiver
    /// * `timelock` optional transaction height lock
    pub fn d_spend_coins(
        &mut self,
        inputs: Vec<MWCoin>,
        mut slate: Slate,
        fund_value: u64,
        timelock: u64,
    ) -> Result<SpendCoinsResult, String> {
        // Validate output coin rangeproofs
        let tx = slate.tx.clone().unwrap_or_else(|| Transaction::empty());
        for out in tx.outputs() {
            let prf = out.proof;
            let com = out.identifier.commit;
            self.secp
                .verify_bullet_proof(com, prf, None)
                .expect("Failed to verify outputcoin rangeproof");
        }

        // Some more validations
        if slate.amount != fund_value {
            Err(String::from("Transaction amount found to be invalid"))
        } else if tx.inputs().len() != inputs.len() {
            Err(String::from(
                "Inputs don't match with coins given in parameters",
            ))
        } else {
            // Validate Kernel features (transaction lock height)
            let valid_features;
            if timelock != 0 {
                valid_features = slate.kernel_features == 2
                    && slate.kernel_features_args.clone().unwrap().lock_height == timelock;
            } else {
                valid_features = slate.kernel_features == 0;
            }
            if !valid_features {
                Err(String::from("Transaction timelock is not setup correctly!"))
            } else {
                // Now we create the signing keys for this participant
                let mut blind_sum = BlindSum::new();
                for coin in inputs {
                    let inp_key = deserialize_secret_key(&coin.blinding_factor, &self.secp);
                    let inp_bf = BlindingFactor::from_secret_key(inp_key.clone());
                    blind_sum = blind_sum.sub_blinding_factor(inp_bf);
                }
                let final_key = self
                    .chain
                    .blind_sum(&blind_sum)
                    .expect("Failed to calculate final blinding factor sum")
                    .secret_key(&self.secp)
                    .unwrap();
                let sig_nonce = create_secret_key(&mut self.rng, &self.secp);
                let mut ctx: Context = Context {
                    parent_key_id: Identifier::zero(),
                    sec_key: final_key.clone(),
                    sec_nonce: sig_nonce.clone(),
                    initial_sec_key: final_key.clone(),
                    initial_sec_nonce: sig_nonce.clone(),
                    output_ids: vec![],
                    input_ids: vec![],
                    amount: fund_value.clone(),
                    fee: Some(slate.fee_fields),
                    payment_proof_derivation_index: None,
                    late_lock_args: None,
                    calculated_excess: None,
                };
                slate
                    .fill_round_1(&self.chain, &mut ctx)
                    .expect("Failed to complete round 1 on the senders turn");
                Ok(SpendCoinsResult {
                    slate: slate,
                    sig_key: final_key.clone(),
                    sig_nonce: sig_nonce.clone(),
                    change_coin: None,
                })
            }
        }
    }

    /// Implementation of the receive coins algorithm of the thesis
    /// Returns an updated pre-transaction (slate) with one partial signature added
    /// and a spendable output coins
    ///
    /// # Arguments
    ///
    /// * `slate` the pre-transaction slate as received from the sender
    /// * `fund_value` the value that should be transferred to the reciever (IN NANO GRIN)
    pub fn recv_coins(
        &mut self,
        mut slate: Slate,
        fund_value: u64,
    ) -> Result<RecvCoinsResult, String> {
        // Validate output coin rangeproofs
        let mut tx = slate.tx.unwrap_or_else(|| Transaction::empty());
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
        let mut ctx = create_minimal_ctx(
            out_coin_key.clone(),
            sig_nonce.clone(),
            fund_value,
            slate.fee_fields,
        );
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

    /// Implementation of the adapted variant of the recv coins algorithm
    /// takes the secret value x and hides it in the adapted signature
    /// The other participant can then verify that the signature contains
    /// x by knowing X and adding it to the public key
    /// Finally the other participant will be able to extract x
    /// from the final signature
    ///
    /// # Arguments
    /// * `slate` the updated slate as received by the sender
    /// * `fund_value` the transaction value in nanogrin
    /// * `x` the secret to hide in the signature
    pub fn apt_recv_coins(
        &mut self,
        mut slate: Slate,
        fund_value: u64,
        x: SecretKey,
    ) -> Result<AptRecCoinsResult, String> {
        // Validate output coin rangeproofs
        let mut tx = slate.tx.unwrap_or_else(|| Transaction::empty());
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
        let mut ctx = create_minimal_ctx(
            out_coin_key.clone(),
            sig_nonce.clone(),
            fund_value,
            slate.fee_fields,
        );
        slate
            .fill_round_1(&self.chain, &mut ctx)
            .expect("Failed to complete round 1 on receivers turn");

        let pub_nonce_sum = slate.pub_nonce_sum(&self.secp).unwrap();
        let pub_blind_sum = slate.pub_blind_sum(&self.secp).unwrap();
        let msg = slate.msg_to_sign().unwrap();
        // Signs the transaction
        let sig = aggsig::sign_single(
            &self.secp,
            &msg,
            &out_coin_key,
            Some(&sig_nonce),
            None,
            Some(&pub_nonce_sum),
            Some(&pub_blind_sum),
            Some(&pub_nonce_sum),
        )
        .expect("Failed to calculate adapted signature in apt_recv");
        let apt_sig = aggsig::sign_single(
            &self.secp,
            &msg,
            &out_coin_key,
            Some(&sig_nonce),
            Some(&x),
            Some(&pub_nonce_sum),
            Some(&pub_blind_sum),
            Some(&pub_nonce_sum),
        )
        .expect("Failed to calculate unadapted signature in apt_recv");

        // Add the adapted signature
        let pub_excess = PublicKey::from_secret_key(&self.secp, &out_coin_key).unwrap();
        let pub_nonce = PublicKey::from_secret_key(&self.secp, &sig_nonce).unwrap();
        for i in 0..slate.num_participants() as usize {
            // find my entry
            if slate.participant_data[i].public_blind_excess == pub_excess
                && slate.participant_data[i].public_nonce == pub_nonce
            {
                slate.participant_data[i].part_sig = Some(apt_sig);
                break;
            }
        }
        Ok(AptRecCoinsResult {
            slate: slate,
            output_coin: MWCoin::new(&commitment, &out_coin_key, fund_value),
            prt_sig: (apt_sig, sig),
        })
    }

    /// Implementation of the dRecvCoins Algorithm round one
    /// In this round the first participant create his partial commitment,
    /// initiliazes the rangeproof protocol (running the first round there),
    /// and adding his participant data to the transaction slate.
    /// This round needs to be called only once by the participant who received the
    /// slate from the sender.
    /// The functions returns an updated slate, the proof context and blinding factor +
    /// nonce values
    ///
    /// # Arguments
    /// * `slate` the slate as received from the sender
    /// * `fund_value` amount of funds which should be received (in nanogrin)
    pub fn drecv_coins_r1(
        &mut self,
        mut slate: Slate,
        fund_value: u64,
    ) -> Result<DRecvCoinsResult, String> {
        // Validate senders output coins
        let tx = slate.tx.clone().unwrap_or_else(|| Transaction::empty());
        for out in tx.outputs() {
            let prf = out.proof;
            let com = out.identifier.commit;
            self.secp
                .verify_bullet_proof(com, prf, None)
                .expect("Failed to verify outputcoin rangeproof");
        }

        let out_coin_blind = create_secret_key(&mut self.rng, &self.secp);
        let shared_nonce = create_secret_key(&mut self.rng, &self.secp);
        let prf_nonce = create_secret_key(&mut self.rng, &self.secp);
        let sig_nonce = create_secret_key(&mut self.rng, &self.secp);

        // Fill participant data
        slate
            .fill_round_1(
                &self.chain,
                &mut create_minimal_ctx(
                    out_coin_blind.clone(),
                    sig_nonce.clone(),
                    fund_value,
                    slate.fee_fields,
                ),
            )
            .expect("Faile to fill_round_1 on drecv_coins r1");

        // Create partial commitment for the output coin and initiate multiparty rangeproof
        let com = self
            .secp
            .commit(fund_value, out_coin_blind.clone())
            .expect("Failed to generate pedersen commitment for drecv_coins_r1");
        let mut prf_ctx = MPBPContext::new(shared_nonce, fund_value, com);
        prf_ctx = mp_bullet_proof_r1(prf_ctx, out_coin_blind.clone(), prf_nonce.clone())
            .expect("Failed to run round 1A of mp bulletproofs");
        // Add the partial signature
        Ok(DRecvCoinsResult {
            slate: slate,
            out_key_blind: out_coin_blind.clone(),
            sig_nonce: sig_nonce.clone(),
            prf_nonce: prf_nonce.clone(),
            prf_ctx: prf_ctx,
        })
    }

    /// Implementation of the dRecvCoins algorithm round 2
    /// In this round the second participant (having received the output of round 1
    /// of this protocol) adds his data to transaction slate, runs round 1 and round 2
    /// of the multiparty bulletproof protocol and adds his commitment to the
    /// output coin commitment, as well as his partial signature. This round of the protocol only needs to be run
    /// one single time by the party who did not run round 1.
    /// The function returns an updated slate, the participants share of the output coins
    /// and an updated multiparty rangeproof contex.
    ///
    /// # Arguments
    /// * `slate` the slate as returned from the call to round 1 of the protocol
    /// * `fund_value` the value of the output coin
    /// * `prf_ctx` the mutliparty bulletproof context
    pub fn drecv_coins_r2(
        &mut self,
        mut slate: Slate,
        fund_value: u64,
        mut prf_ctx: MPBPContext,
    ) -> Result<(RecvCoinsResult, MPBPContext), String> {
        // Validate senders output coins
        let tx = slate.tx.clone().unwrap_or_else(|| Transaction::empty());
        for out in tx.outputs() {
            let prf = out.proof;
            let com = out.identifier.commit;
            self.secp
                .verify_bullet_proof(com, prf, None)
                .expect("Failed to verify outputcoin rangeproof");
        }

        // Add our share to the coin commitment created by receiver 1
        let out_coin_blind = create_secret_key(&mut self.rng, &self.secp);
        let prf_nonce = create_secret_key(&mut self.rng, &self.secp);
        let sig_nonce = create_secret_key(&mut self.rng, &self.secp);

        let com = self
            .secp
            .commit(0, out_coin_blind.clone())
            .expect("Failed to generete pedersen commitment for drecv_coins_r2");
        prf_ctx.add_commit(com);
        prf_ctx = mp_bullet_proof_r1(prf_ctx, out_coin_blind.clone(), prf_nonce.clone())
            .expect("Failed to run round 1B of mp bulletproofs");
        // T1 and T2 and the commitment are now finalized we can start round 2
        prf_ctx = mp_bullet_proof_r2(prf_ctx, out_coin_blind.clone(), prf_nonce.clone())
            .expect("Failed to run round 2A of mp bulletproofs");

        slate
            .fill_round_1(
                &self.chain,
                &mut create_minimal_ctx(
                    out_coin_blind.clone(),
                    sig_nonce.clone(),
                    fund_value,
                    slate.fee_fields,
                ),
            )
            .expect("Failed to run fill_round_1 on drecv_coins_r2");

        // Now we are ready to create the first partial signature
        slate
            .fill_round_2(&self.chain, &out_coin_blind, &sig_nonce)
            .expect("Failed to run fill_round_2 on drecv_coins_r2");

        let coin = MWCoin::new(&com, &out_coin_blind, fund_value);
        Ok((
            RecvCoinsResult {
                slate: slate,
                output_coin: coin,
            },
            prf_ctx,
        ))
    }

    /// Implementation of the dRecvCoins protocol round 3.
    /// This round needs to be run by the participant who run round 1 and
    /// not the second participant who ran round 2.
    /// It will finalize the output coins rangeproof, add the final output
    /// coin to the transaction and add the participants partial signature
    /// it returns an updated transaction slate which can be returned
    /// to the sender
    ///
    /// # Arguments
    /// * `slate` updated slate as of after running round 2 of the protocol
    /// * `prf_ctx` updated proof context as of after running round 2 of the protocol
    /// * `fund_value` the fund value of the output coin
    /// * `out_coin_blind` share of the output coin blinding factor
    /// * `prf_nonce` the nonce used in the rangeproof
    /// * `sig_nonce` the nonce used for the signature creation
    pub fn drecv_coins_r3(
        &mut self,
        mut slate: Slate,
        mut prf_ctx: MPBPContext,
        out_coin_blind: SecretKey,
        prf_nonce: SecretKey,
        sig_nonce: SecretKey,
    ) -> Result<RecvCoinsResult, String> {
        let commit = prf_ctx.commit.clone();
        let amount = prf_ctx.amount.clone();
        // Run round 2 of the the mp bulletproof protocol
        prf_ctx = mp_bullet_proof_r2(prf_ctx, out_coin_blind.clone(), prf_nonce.clone())
            .expect("failed to run round 2 of mp_bullet_proof");
        // Finalize the bulletproof
        let proof = mp_bullet_proof_fin(prf_ctx, out_coin_blind.clone(), prf_nonce.clone())
            .expect("Failed to finalize mp bulletproof");
        let output = Output::new(OutputFeatures::Plain, commit, proof);
        let mut tx = slate.tx.unwrap();
        tx = tx.with_output(output);
        slate.tx = Some(tx);
        slate
            .fill_round_2(&self.chain, &out_coin_blind.clone(), &sig_nonce.clone())
            .unwrap();

        slate
            .update_kernel()
            .expect("Failed to update kernel in drecv_coins_r3");
        let out_coin = MWCoin::new(&commit, &out_coin_blind, amount);
        Ok(RecvCoinsResult {
            slate: slate,
            output_coin: out_coin,
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
    /// * `finalize` if we should finalize the transaction (set it to false if there are further signatures coming i.e. in the dSpendCoins case)
    /// * `pub_x` if we are in dAptFinTx and verify the partial signature with the pub_x as extra data
    /// * `pt_sig` if we are in dAptFinTx and want to replace the receivers adapted signature with the unadapted one
    pub fn fin_tx(
        &mut self,
        mut slate: Slate,
        sec_key: &SecretKey,
        sec_nonce: &SecretKey,
        finalize: bool,
        pub_x: Option<PublicKey>,
        replace_sig: Option<(Signature, Signature)>,
    ) -> Result<Slate, String> {
        // First we verify output coin rangeproofs
        let tx = slate.tx.clone().unwrap();
        for out in tx.outputs() {
            let prf = out.proof;
            let com = out.identifier.commit;
            self.secp
                .verify_bullet_proof(com, prf, None)
                .expect("Failed to verify outputcoin rangeproof");
        }
        if pub_x.is_some() {
            let pub_nonce_sum = slate.pub_nonce_sum(&self.secp).unwrap();
            let pub_blind_sum = slate.pub_blind_sum(&self.secp).unwrap();
            let msg = slate.msg_to_sign().unwrap();
            // In the dAptFinTx we can't use fill_round_2 because we need to verify the adapted pt sig
            for p in slate.participant_data.iter() {
                if p.is_complete() {
                    if !aggsig::verify_single(
                        &self.secp,
                        &p.part_sig.unwrap(),
                        &msg,
                        Some(&pub_nonce_sum),
                        &p.public_blind_excess,
                        Some(&pub_blind_sum),
                        Some(&pub_x.unwrap()),
                        true,
                    ) {
                        panic!("Partial adapted signature verification failed");
                    }
                }
            }
            // Signs the transaction
            let sig = aggsig::sign_single(
                &self.secp,
                &msg,
                &sec_key,
                Some(&sec_nonce),
                None,
                Some(&pub_nonce_sum),
                Some(&pub_blind_sum),
                Some(&pub_nonce_sum),
            )
            .expect("Failed to calculate signature in fin_tx");

            // Add the signature
            let pub_excess = PublicKey::from_secret_key(&self.secp, &sec_key).unwrap();
            let pub_nonce = PublicKey::from_secret_key(&self.secp, &sec_nonce).unwrap();
            for i in 0..slate.num_participants() as usize {
                // find my entry
                if slate.participant_data[i].public_blind_excess == pub_excess
                    && slate.participant_data[i].public_nonce == pub_nonce
                {
                    slate.participant_data[i].part_sig = Some(sig);
                    break;
                }
            }
        } else {
            // Replace adapted signature with the unadapted one before transaction completion
            if replace_sig.is_some() {
                let old_sig = replace_sig.unwrap().0;
                let new_sig = replace_sig.unwrap().1;
                for i in 0..slate.num_participants() as usize {
                    if slate.participant_data[i].part_sig == Some(old_sig) {
                        println!("Replacing that sig");
                        slate.participant_data[i].part_sig = Some(new_sig);
                        break;
                    }
                }
            }

            slate
                .fill_round_2(&self.chain, sec_key, sec_nonce)
                .expect("Failed to complete round 2 on senders turn");
        }
        if finalize {
            slate
                .finalize(&self.chain)
                .expect("Failed to finalize transaction");
        }
        Ok(slate)
    }

    /// Extract Secret Witness value from two partial signatures
    /// Essentially calculates the difference in s between prt_sig and apt_sig
    /// Return the x as SecretKey as hidden in an adapted signature
    ///
    /// # Arguments
    /// * `prt_sig` the unadapted partial signature (does not hold the x)
    /// * `apt_sig` the adapted signature (holds the x)
    pub fn ext_witness(&mut self, prt_sig: Signature, apt_sig: Signature) -> SecretKey {
        let mut apt_s = sig_extract_s(&apt_sig, &self.secp);
        let mut prt_s = sig_extract_s(&prt_sig, &self.secp);
        prt_s.neg_assign(&self.secp).unwrap();
        apt_s.add_assign(&self.secp, &prt_s).unwrap();
        apt_s
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
    use grin_util::{secp::PublicKey, RwLock};
    use grin_wallet_libwallet::{Slate, Slatepacker, SlatepackerArgs};

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

        let result = core.spend_coins(vec![coin], fund_value, 0, 2, 2).unwrap();
        let ser = serde_json::to_string(&result.slate).unwrap();
        let tx = result.slate.tx.unwrap();
        let fee: u64 = result.slate.fee_fields.fee(0);
        assert_eq!(
            input_val - fund_value - fee,
            result.change_coin.unwrap().value
        );
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
        core.spend_coins(vec![], fund_value, 0, 2, 2).unwrap();
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
        core.spend_coins(vec![coin], fund_value, 0, 2, 2).unwrap();
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
        core.spend_coins(vec![coin], fund_value, 0, 2, 2).unwrap();
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
        core.spend_coins(vec![coin, coin2], fund_value, 0, 2, 2)
            .unwrap();
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
        let fin_slate = core.fin_tx(slate, &sk, &nonce, true, None, None).unwrap();
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
        core.fin_tx(slate, &sk, &nonce, true, None, None).unwrap();
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
        core.fin_tx(slate, &sk, &nonce, true, None, None).unwrap();
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
        let result1 = core.spend_coins(vec![coin], fund_value, 0, 2, 2).unwrap();
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
        let fin_slate = core
            .fin_tx(result2.slate, &sec_key, &sec_nonce, true, None, None)
            .unwrap();
        let ser = serde_json::to_string(&fin_slate).unwrap();
        println!("final slate: {}", ser);
    }

    #[test]
    fn test_full_tx_flow_timelock() {
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
        let timelock : u64 = 1052054;

        set_local_chain_type(ChainTypes::AutomatedTesting);
        let result1 = core.spend_coins(vec![coin], fund_value, timelock, 2, 2).unwrap();
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
        let fin_slate = core
            .fin_tx(result2.slate, &sec_key, &sec_nonce, true, None, None)
            .unwrap();
        let ser = serde_json::to_string(&fin_slate).unwrap();
        println!("final slate: {}", ser);
    }

    #[test]
    fn test_full_flow_dspend() {
        set_local_chain_type(ChainTypes::AutomatedTesting);
        let fund_value = grin_to_nanogrin(2);
        let mut core = GrinCore::new();

        // Create some valid input coin
        let input_val = fund_value * 2;
        let bf_a = create_secret_key(&mut core.rng, &core.secp);
        let bf_b = create_secret_key(&mut core.rng, &core.secp);
        let commit_a = core.secp.commit(input_val, bf_a.clone()).unwrap();
        let commit_b = core.secp.commit(0, bf_b.clone()).unwrap();
        let commit = core
            .secp
            .commit_sum(vec![commit_a, commit_b], vec![])
            .unwrap();
        let coin_a = MWCoin {
            commitment: serialize_commitment(&commit),
            blinding_factor: serialize_secret_key(&bf_a),
            value: input_val,
        };
        let coin_b = MWCoin {
            commitment: serialize_commitment(&commit),
            blinding_factor: serialize_secret_key(&bf_b),
            value: input_val,
        };
        let result1 = core.spend_coins(vec![coin_a], fund_value, 0, 2, 3).unwrap();
        let result2 = core
            .d_spend_coins(vec![coin_b], result1.slate, fund_value, 0)
            .unwrap();
        let result3 = core.recv_coins(result2.slate, fund_value).unwrap();
        let result4 = core
            .fin_tx(
                result3.slate,
                &result1.sig_key,
                &result1.sig_nonce,
                false,
                None,
                None,
            )
            .unwrap();
        let fin_slate = core
            .fin_tx(
                result4,
                &result2.sig_key,
                &result2.sig_nonce,
                true,
                None,
                None,
            )
            .unwrap();
        let ser = serde_json::to_string(&fin_slate).unwrap();
        println!("final slate: {}", ser);
    }

    #[test]
    fn test_full_tx_flow_drecv() {
        set_local_chain_type(ChainTypes::AutomatedTesting);
        let fund_value = grin_to_nanogrin(2);
        let mut core = GrinCore::new();

        // Create some valid input coin
        let inp_val = fund_value * 2;
        let inp_bf = create_secret_key(&mut core.rng, &core.secp);
        let commitment = core.secp.commit(inp_val, inp_bf.clone()).unwrap();
        let coin = MWCoin {
            commitment: serialize_commitment(&commitment),
            blinding_factor: serialize_secret_key(&inp_bf),
            value: inp_val,
        };

        let result1 = core.spend_coins(vec![coin], fund_value, 0, 2, 3).unwrap();
        let result2 = core.drecv_coins_r1(result1.slate, fund_value).unwrap();
        let result3 = core
            .drecv_coins_r2(result2.slate, fund_value, result2.prf_ctx)
            .unwrap();
        let recv_coins_res = result3.0;
        let prf_ctx = result3.1;
        let result4 = core
            .drecv_coins_r3(
                recv_coins_res.slate,
                prf_ctx,
                result2.out_key_blind,
                result2.prf_nonce,
                result2.sig_nonce,
            )
            .unwrap();
        let fin_slate = core
            .fin_tx(
                result4.slate,
                &result1.sig_key,
                &result1.sig_nonce,
                true,
                None,
                None,
            )
            .unwrap();
        let ser = serde_json::to_string(&fin_slate).unwrap();
        println!("final slate: {}", ser);
    }

    #[test]
    fn test_full_flow_apt() {
        set_local_chain_type(ChainTypes::AutomatedTesting);
        let fund_value = grin_to_nanogrin(2);
        let mut core = GrinCore::new();

        // Create some valid input coin
        let input_val = fund_value * 2;
        let bf_a = create_secret_key(&mut core.rng, &core.secp);
        let bf_b = create_secret_key(&mut core.rng, &core.secp);
        let commit_a = core.secp.commit(input_val, bf_a.clone()).unwrap();
        let commit_b = core.secp.commit(0, bf_b.clone()).unwrap();
        let commit = core
            .secp
            .commit_sum(vec![commit_a, commit_b], vec![])
            .unwrap();
        let coin_a = MWCoin {
            commitment: serialize_commitment(&commit),
            blinding_factor: serialize_secret_key(&bf_a),
            value: input_val,
        };
        let coin_b = MWCoin {
            commitment: serialize_commitment(&commit),
            blinding_factor: serialize_secret_key(&bf_b),
            value: input_val,
        };
        let result1 = core.spend_coins(vec![coin_a], fund_value, 0, 2, 3).unwrap();
        let result2 = core
            .d_spend_coins(vec![coin_b], result1.slate, fund_value, 0)
            .unwrap();
        // Hide a secret x
        let x = create_secret_key(&mut core.rng, &core.secp);
        let pub_x = PublicKey::from_secret_key(&core.secp, &x).unwrap();
        let result3 = core
            .apt_recv_coins(result2.slate, fund_value, x.clone())
            .unwrap();
        let result4 = core
            .fin_tx(
                result3.slate,
                &result1.sig_key,
                &result1.sig_nonce,
                false,
                Some(pub_x),
                None,
            )
            .unwrap();
        let fin_slate = core
            .fin_tx(
                result4,
                &result2.sig_key,
                &result2.sig_nonce,
                true,
                None,
                Some(result3.prt_sig),
            )
            .unwrap();
        let ser = serde_json::to_string(&fin_slate).unwrap();
        println!("final slate: {}", ser);
        // Extract x from final transaction
        let x_2 = core.ext_witness(result3.prt_sig.1, result3.prt_sig.0);
        assert_eq!(x, x_2);
    }

    #[test]
    fn read_from_slatepack() {
        set_local_chain_type(ChainTypes::AutomatedTesting);
        let mut core = GrinCore::new();
        let slatepack_str = String::from(
            r#"BEGINSLATEPACK. 2ggbW8PZi5GV1Y5 KTMXbjZUbLTxNDx c2uEp9Jg8F9Qq5i z34zazcL2tA6tHb bqQ3PdSnpEoGEBu fYdLDupQt7psw7q CvC4z7a2c9hXHNa dK4v3YHjKJu6csy LfUYQyQZR5NrtN6 XXDeeEnJyG4xxmZ J2uKaWM4wPBgFsD 6kNN7LbZWKASmMu mLDEJuBhUk1M3Jg 8PQnLvc1UMppzeE jecBThZMWjckUZM yAPzyUqxJSjfqcA BDNfPaFLHbrjCW8 uakuAubanRcXVon tSXn54ikJHd4FdH sZWbnzxu7j2ddG8 J5txMJas. ENDSLATEPACK."#,
        );
        let packer = Slatepacker::new(SlatepackerArgs {
            sender: None,
            recipients: vec![],
            dec_key: None,
        });
        let slatepack = packer
            .deser_slatepack(&slatepack_str.into_bytes(), false)
            .unwrap();
        let slate = packer.get_slate(&slatepack).unwrap();
        let slate_str = serde_json::to_string(&slate).unwrap();
        println!("decoded slate: {}", slate_str);
        let result = core.recv_coins(slate, grin_to_nanogrin(2)).unwrap();
        let ser = serde_json::to_string(&result.slate).unwrap();
        println!("final slate: {}", ser);
        println!(
            "commitment: {}, blinding factor: {}, value: {}",
            &result.output_coin.commitment,
            &result.output_coin.blinding_factor,
            result.output_coin.value
        );
        let upt_slatepack = packer.create_slatepack(&result.slate).unwrap();
        let ser_pack = serde_json::to_string(&upt_slatepack).unwrap();
        println!("slatepack: {}", &ser_pack);
    }

    #[test]
    fn test_full_tx_flow_slatepack() {
        let fund_value = 50000000;
        let mut core = GrinCore::new();

        // here is some actually valid input coin (at least was at some point)
        let coin = MWCoin {
            commitment: String::from("09257c975816e6ba6e9a66d1956a202b80d2cd25889a6bef2db0542d51fad6df8e"),
            blinding_factor: String::from("afa38b309656a60024064b045ce30209c7fd5d406aa2e9216b74287f7425da41"),
            value: 2000000000,
        };

        set_local_chain_type(ChainTypes::AutomatedTesting);
        let result1 = core.spend_coins(vec![coin], fund_value, 0, 2, 2).unwrap();
        let result2 = core.recv_coins(result1.slate, fund_value).unwrap();
        let sec_key = result1.sig_key;
        let sec_nonce = result1.sig_nonce;
        let fin_slate = core
            .fin_tx(result2.slate, &sec_key, &sec_nonce, true, None, None)
            .unwrap();
        let ser = serde_json::to_string(&fin_slate).unwrap();
        
        let change_coin = result1.change_coin.unwrap();
        let out_coin = result2.output_coin;
        println!(
            "Sender change coin: commitment: {}, blinding factor: {}, value: {}",
            &change_coin.commitment,
            &change_coin.blinding_factor,
            &change_coin.value
        );
        println!("Receiver output coin: commitment: {}, blinding factor: {}, value: {}",
            &out_coin.commitment,
            &out_coin.blinding_factor,
            &out_coin.value
        );
        let packer = Slatepacker::new(SlatepackerArgs {
            sender: None,
            recipients: vec![],
            dec_key: None,
        });
        let upt_slatepack = packer.create_slatepack(&fin_slate).unwrap();
        let ser_pack = serde_json::to_string(&upt_slatepack).unwrap();
        println!("slatepack: {}", &ser_pack);
    }
}
