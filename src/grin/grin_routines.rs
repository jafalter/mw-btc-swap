use rand::rngs::OsRng;
use grin_util::secp::Secp256k1;
use grin_util::secp::SecretKey;
use grin_util::secp::pedersen::Commitment;

use crate::constants::NANO_GRIN;

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