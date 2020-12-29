use rand::rngs::OsRng;
use grin_util::secp::Secp256k1;
use grin_util::secp::SecretKey;

/// Randomly generate a new secrety key which can be used in commitments and proofs
/// 
/// # Arguments
/// * `rng` randomness generator
/// * `secp` Elliptic curve functionalities
pub fn create_secret_key(rng : &mut OsRng, secp : &Secp256k1) -> SecretKey {
    SecretKey::new(secp, rng)
}

/// Clones a SecretKey
pub fn clone_secret_key(sk : &SecretKey) -> SecretKey {
    SecretKey {
        0 : sk.0
    }
}