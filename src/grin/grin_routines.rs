use rand::rngs::OsRng;
use grin_util::secp::{PublicKey, Secp256k1, key::ZERO_KEY, pedersen::RangeProof};
use grin_util::secp::SecretKey;
use grin_util::secp::pedersen::Commitment;

use crate::constants::NANO_GRIN;

pub struct MPBPContext {
    t_1 : PublicKey,
    t_2 : PublicKey,
    shared_nonce : PublicKey,
    tau_x : SecretKey,
    rng: OsRng,
    secp: Secp256k1
}

impl MPBPContext {

    fn new(shared_nonce : PublicKey) -> MPBPContext {
        let rng = get_os_rng();
        let secp = Secp256k1::with_caps(ContextFlag::Commit);
        MPBPContext {
            t_1 : PublicKey::new(),
            t_2 : PublicKey::new(),
            shared_nonce : shared_nonce,
            tau_x : ZERO_KEY,
            rng : rng,
            secp : secp
        }
    }

    fn add_t_1(&mut self, t_1 : PublicKey) {
        self.t_1.add_exp_assign(self.secp, t_1);
    }

    fn add_t_2(&mut self, t_2: PublicKey) {
        self.t_2.add_exp_assign(self.secp, t_2);
    }

    fn add_tau_x(&mut self, tau_x: SecretKey) {
        self.tau_x.add_assign(self.secp, tau_x);
    } 
}

/// Randomly generate a new secrety key which can be used in commitments and proofs
/// 
/// # Arguments
/// * `rng` randomness generator
/// * `secp` Elliptic curve functionalities
pub fn create_secret_key(rng : &mut OsRng, secp : &Secp256k1) -> SecretKey {
    SecretKey::new(secp, rng)
}

/// Serialze a secret key to a hex encoded string
/// 
/// # Arguments
/// * `key` the key to serialize
pub fn serialize_secret_key(key : &SecretKey) -> String {
    hex::encode(&key)
}

/// Deserialize a hexdecoded string to a SecretKey object
/// 
/// # Arguments
/// * `key` the key serialized as hex string
/// * `secp` Secp256 functionatlity
pub fn deserialize_secret_key(key : &String, secp : &Secp256k1) -> SecretKey {
    SecretKey::from_slice(secp, &hex::decode(key)
        .expect("Failed to deserialize a secret key from hex string"))
        .expect("Failed to deserialize a secret key from hex string")
}

/// Serialize a pedersen commitment to a hex encoded string
/// 
/// # Arguments
/// * `com` pedersen commitment instance
pub fn serialize_commitment(com : &Commitment) -> String {
    hex::encode(&com)
}

/// Deserialize a pedersen commitment from a hex encoded string
/// 
/// # Arguments
///
/// * `com` commitment encoded as hex string
pub fn deserialize_commitment(com : &String) -> Commitment {
    Commitment::from_vec(hex::decode(com)
        .expect("Failed to deserialize pedersen commitment"))
}

/// Conversion from grin to nanogrin
/// 
/// # Arguments
///
/// * `value` value in grins
pub fn grin_to_nanogrin(value : u64) -> u64 {
    value * NANO_GRIN
}

pub fn mp_bullet_proof_r1(ctx : MPBPContext, blind: &SecretKey, amount : u64, sec_nonce : SecretKey) -> Result<MPBPContext, String> {
    // These will be filled during the round
    let mut t_1= PublicKey::new();
    let mut t_2: PublicKey::new();
    // Create a commitment to 0
    let commit = ctx.secp.commit(0, create_secret_key(ctx.rng, ctx.secp));
    // If not already done create a shared nonce
    ctx.secp.bullet_proof_multisig(
        amount, 
        blind, 
        ctx.shared_nonce.clone(), 
        None, 
        None, 
        None, 
        Some(&mut t_1),
        Some(&mut t_2), 
        vec![commit], 
        sec_nonce, 
        1);
    ctx.add_t_1(&t_1);
    ctx.add_t_2(&t_2);
    Ok(ctx)
}

pub fn mp_bullet_proof_r2(ctx : MPBPContext, blind : &SecretKey, amount : u64, commit : Commitment, sec_nonce : SecretKey) -> Result<MPBPContext, String> {
    let mut tau_x = create_secret_key(ctx.rng, ctx.secp);
    ctx.secp.bullet_proof_multisig(
        amount, 
        blind, 
        ctx.shared_nonce, 
        None, 
        None, 
        Some(&mut tau_x), 
        Some(&mut ctx.t_1.clone()), 
        Some(&mut ctx.t_2.clone()), 
        vec![commit], 
        sec_nonce, 
        2);
    ctx.add_tau_x(tau_x);
    Ok(ctx)
}

pub fn mp_bullet_proof_fin(ctx : MPBPContext, blind : &SecretKey, amount : u64, commit : Commitment, sec_nonce : SecretKey) -> Result<RangeProof, String> {
    let proof = ctx.secp.bullet_proof_multisig(
        amount, 
        blind, 
        ctx.shared_nonce, 
        None, 
        None, 
        ctx.tau_x, 
        ctx.t_1, 
        ctx.t_2, 
        vec![commit], 
        sec_nonce, 
        0)
            .expect("Failed to finalize MP bulletproof");
    ctx.secp.verify_bullet_proof(commit, proof, None)
        .expect("MP Bulletproof is invalid");
    Ok(proof)
}

#[cfg(test)]
mod test {
    use grin_util::secp::{ContextFlag, Secp256k1};

    use crate::util::get_os_rng;

    use super::{create_secret_key, deserialize_commitment, deserialize_secret_key, serialize_commitment, serialize_secret_key};


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
    fn test_commit_serialization() {
        let mut rng = get_os_rng();
        let secp = Secp256k1::with_caps(ContextFlag::Commit);
        let sk = create_secret_key(&mut rng, &secp);
        let value : u64 = 1000000;
        let commit = secp.commit(value, sk).unwrap();
        let ser = serialize_commitment(&commit);
        let deser = deserialize_commitment(&ser);
        assert_eq!(commit, deser);
    }
}