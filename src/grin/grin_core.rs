use crate::grin::grin_types::MWCoin;
use grin_wallet_libwallet::Slate;
use grin_util::secp::pedersen::Commitment;
use grin_core::core::transaction::OutputFeatures;
use grin_core::core::{Input,Inputs,Output,Transaction};
use rand::rngs::OsRng;
use grin_util::secp::{Secp256k1,ContextFlag};
use grin_util::secp::key::{PublicKey, SecretKey};
use grin_core::core::transaction::FeeFields;
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
    /// * `fund_value` value which should be transferred to a receiver
    /// * `fee` transaction fee
    /// * `timelock` optional transaction timelock
    pub fn spend_coins(&mut self, inputs : Vec<MWCoin>, fund_value : u64, fee : u64, timelock : u32) -> Result<SpendCoinsResult,String> {
        // Initial transaction slate
        let mut slate = Slate::blank(2, false);
        
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
            // Create needed blinding factors and nonce values
            let out_bf = create_secret_key(&mut self.rng, &self.secp);
            let out_bf_com = out_bf.clone();
            let out_bf_prf = out_bf.clone();
            let sig_nonce = create_secret_key(&mut self.rng, &self.secp);
            let prf_nonce = create_secret_key(&mut self.rng, &self.secp);
            let rew_nonce = create_secret_key(&mut self.rng, &self.secp);
            let mut bf_sum = out_bf.clone();

            slate.amount = fund_value;
            let feefield = FeeFields::new(0, fee).unwrap();
            slate.fee_fields = feefield;
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
            slate.tx = Some(tx);
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
            slate.fill_round_1(&self.chain, &mut ctx)
                .expect("Failed to complete round 1 on the senders turn");
    
            Ok(SpendCoinsResult {
                slate : slate,
                sig_key : bf_sum.clone(),
                sig_nonce : sig_nonce.clone(),
                change_coin : MWCoin::new(&commitment, &out_bf, change_value)
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
        let out_bf = create_secret_key(&mut self.rng, &self.secp);
        let out_bf_com = out_bf.clone();
        let out_bf_prf = out_bf.clone();
        let rew_nonce = create_secret_key(&mut self.rng, &self.secp);
        let prf_nonce = create_secret_key(&mut self.rng, &self.secp);
        let sig_nonce = create_secret_key(&mut self.rng, &self.secp);
        
        let commitment = self.secp.commit(fund_value, out_bf_com)
            .expect("Failed to generate pedersen commitment for recv_coins output coin");
        let proof = self.secp.bullet_proof(fund_value, out_bf_prf, rew_nonce, prf_nonce, None, None);
        let output = Output::new(OutputFeatures::Plain, commitment, proof);

        tx = tx.with_output(output);
        slate.tx = Some(tx);
        let mut ctx : Context = Context {
            parent_key_id: Identifier::zero(),
            sec_key: out_bf.clone(),
            sec_nonce: sig_nonce.clone(),
            initial_sec_key: out_bf.clone(),
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
            output_coin : MWCoin::new(&commitment, &out_bf, fund_value)
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
    use crate::grin::{grin_routines::{*}, grin_types::MWCoin};
    use crate::grin::grin_core::GrinCore;
    use grin_wallet_libwallet::Slate;
    use grin_core::global::{set_local_chain_type, ChainTypes};
    
    #[test]
    fn test_spend_coins() {
        // Should create a pre-transaction with transaction value 600
        let mut core = GrinCore::new();
        let coin = MWCoin {
            commitment : String::from("086061571ea044365c81b5232c261866265024bd5c3506b5526d80df0c6c5845c8"),
            blinding_factor : String::from("1682c7950a19dfbc2f2c409ff9517cc72d2bf1ffa9ab1f83746e843461e1112c"),
            value : 50000
        };
        let result = core.spend_coins(vec!(coin), 600, 20, 0).unwrap();
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
        let mut core = GrinCore::new();
        let fund_value = 600;
        let fee = = 20;
        let coin = MWCoin {
            commitment : String::from("086061571ea044365c81b5232c261866265024bd5c3506b5526d80df0c6c5845c8"),
            blinding_factor : String::from("1682c7950a19dfbc2f2c409ff9517cc72d2bf1ffa9ab1f83746e843461e1112c"),
            value : 50000
        };
        let result1 = core.spend_coins(vec!(coin), fund_value, 20, 0).unwrap();
        println!("sec_key: {} nonce: {}", serialize_secret_key(&result1.sig_key), serialize_secret_key(&result1.sig_nonce));
        let result2 = core.recv_coins(result1.slate, fund_value).unwrap();
        let ser = serde_json::to_string(&result2.slate).unwrap();
        println!("{}", ser);
        /**
        set_local_chain_type(ChainTypes::AutomatedTesting);
        let sec_key = deserialize_secret_key(&String::from("f87c62b6b7e57fb00db05755fd3e167f8ddb208f3e9cca08ed5ab2d1c5edc084"), &core.secp);
        let sec_nonce = deserialize_secret_key(&String::from("29aca6fe1f51683721d56fdb3a5168f18fd5e2e34aeef1ba9708ce5ad99c6020"), &core.secp);
        let str_slate = r#"{"ver":"4:3","id":"4146ae0d-d100-4608-8241-0a2e40c9c1ed","sta":"S1","amt":"600","fee":"20","sigs":[{"xs":"0264041944239ebce4fa774469aaeb8990335942b8ba34fd1fd5d8d080a5c40d61","nonce":"02e4fb420a8ba5ddaa31cd6ad1c85c766fb41b887a5f7ad5b9e1cfeff3c6f08a9b"},{"xs":"0379062090e060911a0e71b6ef768b3828f6f5476e6bd6e151455611027f3e6b46","nonce":"02b08a5503bb5b5cc1a0dcdda994e3e4b077f1941752b7385a392012917477c41c","part":"1cc47774911220395a38b7521794f177b0e4e394a9dddca0c15c5bbb03558ab0f048673735088da439ef499233632e727a6f84ba37dd00b852f90e97e23db636"}],"coms":[{"c":"086061571ea044365c81b5232c261866265024bd5c3506b5526d80df0c6c5845c8"},{"c":"0928f0adc04373eda0dcb4a40b447efda1147625627c1d553a526ba7c26b6595cf","p":"bdbd83b31c947b4edb51b4a00e0a12ae682563f44933744673eb6afae97fc1e2b68d0a925704542dfd422786c547f8b7af2ff88ac0177e41c4692adaeaf9ab2b0f9f30d00a2834cea6d4f682dc2a962eac6c53df5167f830271fc052bc87913ffe6399ea6ac13f50bb53ff81232540d0d6bb167db7810620b9af87c537aed11f983f8355f7d6853fabe5d1da513389b8bc475a3dd274a94ab0878039d08dfbe4af834ef53db94e27bd1e7b902a407d3c3321d67b392303a071d0a1cb1c714a76903e69eceb1c0fb79268f3bae2a3264e77fc8ae2e4fdc690622479b87e6e6acfd14992dd34dcfa722be69ffdd65165f76fe4931584b0961d441f0da67c70a8af64fcbe338b38d9244ca2b2aafe16202a866a00f070bf18b3c77d011ff94090406ba823d50cbc9f8a2f2815c6e2cae833b469af9bab1b2b71c2f4283c6da96a99a67f8aedd9ea097707f1b24e3928dc15fb671ee26a7625d31b978274b111025e6c4702de54e01e3339d1aa1690beee14508602d9beedd61732aa64e13f9e8bc9c3e74a468e8fc6107df1fb70631f90d33c51221bc6f9817e7b9ce83d03751a7c1eff29f745dbf00fa409199c6849a3cc212826cfc88e8e0fa99fa182f321e4c037f86255fa82d2cc7fdb1c467ae04a38c581b5a897dbe7f94f136d6c7f506599a4730841dbda4f96037c96888b480a23e10abe2aadcc220890f7dd963f8d6914a473056032210fcbfc31c66b95f1ac3d9e7e525683bbcada6e75c37efa8d3daff189b310d50bc72eeca9cf22591e94bb446d251348230c1f2c3b49fe3a29ecc50874045ffe979d264472e7dd77566f22093f0b9c17dcd703ac6275d82c276971a693963511321d67e93c5aace7a6bdc8064fb669cb0f7a2807db773a370ff9a723119b7b24a3a759b50640a800cc55bed264f62312217f13bc49da00a7f8d382a8b08c"},{"c":"09ace521cf8a98b62efbffbc799ba9721202437026e89593563878a3a52243f9f1","p":"358cae0b9aade44ca427618c23a64cf563cfe32daa32e730383047df2aba8686bf7e4df279374da2b4461220f0225441d160054627926f62396d4073d76814cd0e2924901f48dbe4b8d9489e19cb8b946c64a1e12a1ab108946da35df935976ad2a574b85ef4c575243d83f0285f7a3423cb2bea04a50e5822faf1c78f55983e63e0ada967b2180f90379c38a5e7e4a8544978370f0c39c180a5708b2a2e7f10bc88cbd619800104a642de80b1b3314f1e69503773e038c1478aeab6eda46d669c8527ba6c7917ee7b893f20a1036a6be5deefe561fe1b3acda8101819fbc0597e3c85962e3c89013db61230d34a47ec0dc086cf7f43847a4b345363b481ad2caf8d654a25640ea1a7e87f64b58d3371ff88ac3fa04cf9e2fc573dd39007c68e67cd3b73881dc8e14477398c78c333a2bbbf84302035116d829e86f9ac3fb99b435f6dbcaede5215d962c05e3143bd41e8feef91af0b0cef7f6aebb6c9fad37c3b61006117554b2f56c192080adc13100798b48ff07a18674106331f633af2a40d6e4fa7c3fdb5f85af65819318973ee46e91704c101dadf02b23e2477a7b7dcb5c50462a73ea748079a55a30d7031d3284ffd915114cf84719c4101b59f5370e296544a6697b85f96c411a4d14efa17e014254981699ccc8bbe6bf43edab5784060777ddfe10b6a759c109f91335c82f7e579d1aec8c3bbc131555594a5df23389d3132ea52a4d3db535c53388ac3f495455b34a3f6eec7a5e4d0ad800f07393c8e17b6964ad6d99ff2e73a7de0ce75ab233b22bd029d33829ddddeae6e9789cf6061c726b00276f73cb0bcde221beb21cdd6ede95d6a7b989b168df67d652b5f9acba63e54e836186163efabcbf3230c9fb4566d80037de4d4ad010a2256d52509aebf030b48c30979d23d60520713fff15a9df78c972fedd8cdb58dfc9fb14e9476"}]}"#;
        let slate = Slate::deserialize_upgrade(&str_slate).unwrap();

        let fin_slate = core.fin_tx(slate, &sec_key, &sec_nonce).unwrap();
        let ser = serde_json::to_string(&fin_slate).unwrap();
        println!("{}", ser);
        */
    }
}