use rand::rngs::OsRng;
use grin_util::secp::Secp256k1;
use grin_util::secp::SecretKey;
use grin_util::secp::pedersen::Commitment;
use grin_core::libtx::secp_ser::{as_hex};

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
/// * `com` commitment encoded as hex string
pub fn deserialize_commitment(com : &String) -> Commitment {
    Commitment::from_vec(hex::decode(com)
        .expect("Failed to deserialize pedersen commitment"))
}