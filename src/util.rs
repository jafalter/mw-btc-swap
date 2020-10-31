use rand::rngs::OsRng;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::All;

pub fn get_os_rng() -> OsRng {
    OsRng::new().expect("Unable to initialize OSRNG")
}

pub fn get_secp256k1_curve() -> Secp256k1<All> {
    Secp256k1::new()
}