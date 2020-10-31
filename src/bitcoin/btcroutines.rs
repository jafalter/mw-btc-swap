use rand::rngs::OsRng;
use bitcoin::PrivateKey;
use bitcoin::secp256k1::key::SecretKey;
use crate::constants::TEST_NET;
use bitcoin::network::constants::Network;

pub fn create_private_key(rng : &mut OsRng) -> PrivateKey {
    let skey = SecretKey::new(rng);
    let nw = if TEST_NET { Network::Testnet } else { Network::Bitcoin };
    PrivateKey {
        compressed : true,
        network : nw,
        key : skey
    }
}