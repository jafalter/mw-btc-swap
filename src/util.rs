use rand::rngs::OsRng;

pub fn get_os_rng() -> OsRng {
    OsRng::new().expect("Unable to initialize OSRNG")
}