use grin_core::{core::FeeFields, libtx::tx_fee};
use grin_keychain::Identifier;
use grin_util::secp::{Signature, pedersen::Commitment};
use grin_util::secp::SecretKey;
use grin_util::secp::{
    key::ZERO_KEY,
    pedersen::{ProofMessage, RangeProof},
    ContextFlag, PublicKey, Secp256k1,
};
use grin_wallet_libwallet::{Context, Slate};
use rand::rngs::OsRng;
use serde::{Serialize, Deserialize};
use bitcoin::PublicKey as BTCPublicKey;
use bitcoin::PrivateKey as BTCPrivKey;


use crate::{bitcoin::btcroutines::create_private_key, constants::NANO_GRIN, util::get_os_rng};

pub struct MPBPContext {
    t_1: PublicKey,
    t_2: PublicKey,
    pub amount: u64,
    shared_nonce: SecretKey,
    tau_x: SecretKey,
    rng: OsRng,
    secp: Secp256k1,
    pub commit: Commitment
}

#[derive(Serialize, Deserialize)]
struct SerMPCtx {
    t_1 : String,
    t_2 : String,
    amount : u64,
    shared_nonce : String,
    commit : String,
    tau_x : String
}

impl MPBPContext {
    pub fn new(shared_nonce: SecretKey, amount: u64, com: Commitment) -> MPBPContext {
        let rng = get_os_rng();
        let secp = Secp256k1::with_caps(ContextFlag::Commit);
        MPBPContext {
            t_1: PublicKey::new(),
            t_2: PublicKey::new(),
            amount: amount,
            shared_nonce: shared_nonce.clone(),
            tau_x: ZERO_KEY,
            rng: rng,
            secp: secp,
            commit: com
        }
    }

    pub fn to_string(&mut self) -> String {
        let ser_obj = SerMPCtx {
            t_1 : serialize_grin_public_key(&self.t_1, &self.secp),
            t_2 : serialize_grin_public_key(&self.t_2, &self.secp),
            amount : self.amount,
            shared_nonce : serialize_secret_key(&self.shared_nonce),
            commit : serialize_commitment(&self.commit),
            tau_x : serialize_secret_key(&self.tau_x)
        };
        serde_json::to_string(&ser_obj).unwrap()
    }

    pub fn from_string(str : &String) -> MPBPContext {
        let rng = get_os_rng();
        let secp = Secp256k1::with_caps(ContextFlag::Commit);
        let ser_obj : SerMPCtx = serde_json::from_str(str)
            .unwrap();
        MPBPContext {
            t_1 : deserialize_grin_pub_key(&ser_obj.t_1, &secp),
            t_2 : deserialize_grin_pub_key(&ser_obj.t_2, &secp),
            amount : ser_obj.amount,
            shared_nonce : deserialize_secret_key(&ser_obj.shared_nonce, &secp),
            tau_x : deserialize_secret_key(&ser_obj.tau_x, &secp),
            rng : rng,
            secp : secp,
            commit : deserialize_commitment(&ser_obj.commit)
        }
    }

    pub fn add_commit(&mut self, c2 : Commitment) {
        self.commit = self
            .secp
            .commit_sum(vec![self.commit,c2], vec![])
            .unwrap()
    }

    fn add_t_1(&mut self, t_1: PublicKey) {
        if !self.t_1.is_valid() {
            self.t_1 = t_1.clone();
        }
        else {
            self.t_1 = PublicKey::from_combination(&self.secp, vec![&self.t_1.clone(), &t_1]).unwrap();
        }
    }

    fn add_t_2(&mut self, t_2: PublicKey) {
        if !self.t_2.is_valid() {
            self.t_2 = t_2.clone();
        }
        else {
            self.t_2 = PublicKey::from_combination(&self.secp, vec![&self.t_2.clone(), &t_2]).unwrap();
        }
    }

    fn add_tau_x(&mut self, tau_x: SecretKey) {
        self.tau_x.add_assign(&self.secp, &tau_x).unwrap();
    }
}

/// Randomly generate a new secrety key which can be used in commitments and proofs
///
/// # Arguments
/// * `rng` randomness generator
/// * `secp` Elliptic curve functionalities
pub fn create_secret_key(rng: &mut OsRng, secp: &Secp256k1) -> SecretKey {
    SecretKey::new(secp, rng)
}

/// Extract the s value from a schnorr signature
/// Returns the s value of the signature as a SecretKey
///
/// # Arguments
/// * `sig` the signature from which to extract s
/// * `secp` elliptic cureve functionalities
pub fn sig_extract_s(sig : &Signature, secp: &Secp256k1) -> SecretKey {
    SecretKey::from_slice(secp, &sig.to_raw_data()[32..])
        .unwrap()
}

/// Serialze a secret key to a hex encoded string
///
/// # Arguments
/// * `key` the key to serialize
pub fn serialize_secret_key(key: &SecretKey) -> String {
    hex::encode(&key)
}

/// Deserialize a hexdecoded string to a SecretKey object
///
/// # Arguments
/// * `key` the key serialized as hex string
/// * `secp` Secp256 functionatlity
pub fn deserialize_secret_key(key: &String, secp: &Secp256k1) -> SecretKey {
    if key == "0000000000000000000000000000000000000000000000000000000000000000" {
        ZERO_KEY
    }
    else {
        SecretKey::from_slice(
            secp,
            &hex::decode(key).expect("Failed to deserialize a secret key from hex string"),
        )
        .expect("Failed to deserialize a secret key from hex string")
    }
}

//// Create a minimal context object needed for Slate fill_round_1
///
/// # Arguments
/// * `sec_key` the secret key used by this participant
/// * `sec_nonce` the secrent nonce used by this participant (for the signature)
/// * `amount` transaction amount
/// * `fee` the transaction fees
pub fn create_minimal_ctx(sec_key : SecretKey, sec_nonce : SecretKey, amount: u64, fee: FeeFields) -> Context {
    Context {
        parent_key_id: Identifier::zero(),
        sec_key: sec_key.clone(),
        sec_nonce: sec_nonce.clone(),
        initial_sec_key: sec_key.clone(),
        initial_sec_nonce: sec_nonce.clone(),
        output_ids: vec![],
        input_ids: vec![],
        amount: amount,
        fee: Some(fee),
        payment_proof_derivation_index: None,
        late_lock_args: None,
        calculated_excess: None
    }
}

/// Serialize a pedersen commitment to a hex encoded string
///
/// # Arguments
///
/// * `com` pedersen commitment instance
pub fn serialize_commitment(com: &Commitment) -> String {
    hex::encode(&com)
}

/// Serialize a public key to a hex encoded compressed string
///
/// # Arguments
///
/// * `pk` the public key to serialize
/// * `secp` Secp256k1 engine
pub fn serialize_grin_public_key(pk: &PublicKey, secp: &Secp256k1) -> String {
    hex::encode(&PublicKey::serialize_vec(&pk, secp, true))
}

/// Deserialize a public key from a hex encoded string
///
/// # Arguments
///
/// * `str` the string containing the hex encoded public key
/// * `secp` the Secp256k1 engine
pub fn deserialize_grin_pub_key(str: &String, secp: &Secp256k1) -> PublicKey {
    let pk_bytes = hex::decode(str)
        .unwrap();
    PublicKey::from_slice(secp, &pk_bytes)
        .unwrap()
}

/// Deserialize a pedersen commitment from a hex encoded string
///
/// # Arguments
///
/// * `com` commitment encoded as hex string
pub fn deserialize_commitment(com: &String) -> Commitment {
    Commitment::from_vec(hex::decode(com).expect("Failed to deserialize pedersen commitment"))
}

/// Conversion from grin to nanogrin
///
/// # Arguments
///
/// * `value` value in grins
pub fn grin_to_nanogrin(value: u64) -> u64 {
    value * NANO_GRIN
}

/// Round 1 of the multiparty bulletproof creation
/// In this round the parties do not yet the final coin commitment, however
/// they already know their part of the blinding factor and amount which will be commited to.
/// This is enough to create their part of T1 and T2 values, which we can then later sum up
/// with the second party.
/// The function returns an updated proof context containing their part of the T1 and T2
///
/// # Arguments
///
/// * `ctx` The initial context object of the proof
/// * `blind` The share of the commitment blinding factor
/// * `sec_nonce` A secret nonce used for proof creation
pub fn mp_bullet_proof_r1(
    mut ctx: MPBPContext,
    blind: SecretKey,
    sec_nonce: SecretKey,
) -> Result<MPBPContext, String> {
    // These will be filled during the round
    let mut t_1 = PublicKey::new();
    let mut t_2 = PublicKey::new();
    let c = ctx.secp.commit(0, create_secret_key(&mut ctx.rng, &ctx.secp))
        .unwrap();
    ctx.secp.bullet_proof_multisig(
        ctx.amount,
        blind.clone(),
        ctx.shared_nonce.clone(),
        None,
        None,
        None,
        Some(&mut t_1),
        Some(&mut t_2),
        vec![c],
        Some(&sec_nonce),
        1,
    );
    ctx.add_t_1(t_1);
    ctx.add_t_2(t_2);
    Ok(ctx)
}

/// Round 2 of the multiparty bulletproof creation
/// In this round the parties know the sum of the T1 and T2 and the final commitment, they
/// are now computing their parts of the tau_x which then again gets summed together with the other party
/// The function returns an updated proof context with their part of the tau_x added
///
/// # Arguments
///
/// * `ctx` the proof context already filled with the T1 and T2 values
/// * `blind` the parties share of the blinding factor
/// * `commit` the final commitment
/// * `sec_nonce` the parties secret nonce
pub fn mp_bullet_proof_r2(
    mut ctx: MPBPContext,
    blind: SecretKey,
    sec_nonce: SecretKey,
) -> Result<MPBPContext, String> {
    let mut tau_x = create_secret_key(&mut ctx.rng, &ctx.secp);
    ctx.secp.bullet_proof_multisig(
        ctx.amount,
        blind.clone(),
        ctx.shared_nonce.clone(),
        None,
        None,
        Some(&mut tau_x),
        Some(&mut ctx.t_1.clone()),
        Some(&mut ctx.t_2.clone()),
        vec![ctx.commit],
        Some(&sec_nonce.clone()),
        2,
    );
    ctx.add_tau_x(tau_x.clone());
    Ok(ctx)
}

/// Conversion from a bitcoin PublicKey to a grin one 
/// Conversion is easy because they both use the same 
/// curve and even same library
///
/// # Arguments
///
/// * `bpk` The bitcoin public key
/// * `secp` Secp256k1 functions
pub fn grin_pk_from_btc_pk(bpk : &BTCPublicKey, secp: &Secp256k1) -> PublicKey {
    PublicKey::from_slice(secp, &bpk.to_bytes())
        .unwrap()
}

/// Concersion from a bitcoin PrivateKey to a grin Secret Key
/// Conversion is easy because they both use the same curve
/// and even same library
///
/// # Arguments
///
/// * `sk` the bitcoin private key
pub fn grin_sk_from_btc_sk(sk : &BTCPrivKey, secp: &Secp256k1) -> SecretKey {
    SecretKey::from_slice(secp, &sk.to_bytes())
        .unwrap()
}

/// The final proof creation algorithm callable be either one of the two parties to creat the final range proof
///
/// # Arguments
///
/// * `ctx` the proof context with the T1 T2 sum as well as the tau_x sum filled
/// * `blind` the parties share of the blinding factor
/// * `commit` the final commitment
/// * `sec_nonce` the parties secret nonce
pub fn mp_bullet_proof_fin(
    mut ctx: MPBPContext,
    blind: SecretKey,
    sec_nonce: SecretKey,
) -> Result<RangeProof, String> {
    let proof = ctx
        .secp
        .bullet_proof_multisig(
            ctx.amount,
            blind.clone(),
            ctx.shared_nonce,
            None,
            None,
            Some(&mut ctx.tau_x),
            Some(&mut ctx.t_1),
            Some(&mut ctx.t_2),
            vec![ctx.commit],
            Some(&sec_nonce.clone()),
            0,
        )
        .expect("Failed to finalize MP bulletproof");
    Ok(proof)
}

pub fn estimate_fees(num_inp : usize, num_out : usize, num_ker : usize) -> u64 {
    tx_fee(num_inp, num_out, num_ker)
}

#[cfg(test)]
mod test {
    use grin_util::secp::{ContextFlag, PublicKey, Secp256k1, SecretKey, pedersen::{Commitment, ProofMessage, ProofRange, RangeProof}};
    use grin_util::secp::Error;

    use crate::util::get_os_rng;

    use super::{MPBPContext, create_secret_key, deserialize_commitment, deserialize_grin_pub_key, deserialize_secret_key, grin_to_nanogrin, mp_bullet_proof_fin, mp_bullet_proof_r1, mp_bullet_proof_r2, serialize_commitment, serialize_grin_public_key, serialize_secret_key};

    #[test]
    fn test_key_serializiation() {
        let mut rng = get_os_rng();
        let secp = Secp256k1::with_caps(ContextFlag::Commit);
        let sk = create_secret_key(&mut rng, &secp);
        let ser = serialize_secret_key(&sk);
        let deser = deserialize_secret_key(&ser, &secp);
        assert_eq!(sk, deser);
    }

    #[test]
    fn test_pub_key_serialization() {
        let mut rng = get_os_rng();
        let secp = Secp256k1::with_caps(ContextFlag::Commit);
        let sk = create_secret_key(&mut rng, &secp);
        let pk = PublicKey::from_secret_key(&secp, &sk)
            .unwrap();
        let ser = serialize_grin_public_key(&pk, &secp);
        let deser = deserialize_grin_pub_key(&ser, &secp);
        assert_eq!(pk, deser);
    }

    #[test]
    fn test_commit_serialization() {
        let mut rng = get_os_rng();
        let secp = Secp256k1::with_caps(ContextFlag::Commit);
        let sk = create_secret_key(&mut rng, &secp);
        let value: u64 = 1000000;
        let commit = secp.commit(value, sk).unwrap();
        let ser = serialize_commitment(&commit);
        let deser = deserialize_commitment(&ser);
        assert_eq!(commit, deser);
    }

    #[test]
    fn test_mp_bulletproof() {
        // Setup
        let mut rng = get_os_rng();
        let secp = Secp256k1::with_caps(ContextFlag::Commit);
        let bf_a = create_secret_key(&mut rng, &secp);
        let bf_b = create_secret_key(&mut rng, &secp);
        let sec_nonce_a = create_secret_key(&mut rng, &secp);
        let sec_nonce_b = create_secret_key(&mut rng, &secp);
        let amount = grin_to_nanogrin(2);
        let shared_nonce = create_secret_key(&mut rng, &secp);
        let commit_a = secp.commit(amount, bf_a.clone())
            .unwrap();
        let mut ctx = MPBPContext::new(shared_nonce, amount, commit_a);

        // Round 1
        ctx = mp_bullet_proof_r1(ctx, bf_a.clone(), sec_nonce_a.clone())
            .unwrap();
        ctx = mp_bullet_proof_r1(ctx, bf_b.clone(), sec_nonce_b.clone())
            .unwrap();

        let commit_b = secp.commit(0, bf_b.clone())
            .unwrap();
        ctx.add_commit(commit_b);
        let com = ctx.commit.clone();

        // Round 2
        let ctx = mp_bullet_proof_r2(ctx, bf_a.clone(), sec_nonce_a.clone())
            .unwrap();
        let ctx = mp_bullet_proof_r2(ctx, bf_b.clone(), sec_nonce_b.clone())
            .unwrap();

        let proof = mp_bullet_proof_fin(ctx, bf_a.clone(), sec_nonce_a.clone())
            .unwrap();

        assert!(secp
            .verify_bullet_proof(com, proof.clone(), None)
            .is_ok());
    }

    #[test]
    pub fn test_deserialize_zero_key() {
        let secp = Secp256k1::with_caps(ContextFlag::Commit);
        let hex = String::from("0000000000000000000000000000000000000000000000000000000000000000");
        let key = deserialize_secret_key(&hex, &secp);
    }

}
