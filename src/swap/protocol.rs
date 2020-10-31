use crate::net::tcp::write_to_stream;
use crate::bitcoin::btcroutines::create_private_key;
use rand::rngs::OsRng;
use std::net::TcpStream;
use crate::SwapSlate;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::All;
use bitcoin::util::psbt::serialize::Serialize;
use hex::encode;

/// Runs the mimblewimble side of the setup phase of the atomic swap
/// 
/// # Arguments
/// 
/// * `slate` reference to the atomic swap slate
pub fn setup_phase_swap_mw(slate : &mut SwapSlate, stream : &mut TcpStream, rng : &mut OsRng, curve : &Secp256k1<All>) -> Result<SwapSlate, &'static str> {

    // Receiver keypair
    let rsk = create_private_key(rng);
    let rpk = rsk.public_key(curve);

    // Send public key to peer
    write_to_stream(stream, &hex::encode(rpk.serialize()));

    Err("Not implemented")
}

/// Runs the bitcoin side of the setup phase of the atomic swap
/// 
/// # Arguments
/// 
/// * `slate` reference to the atomic swap slate
pub fn setup_phase_swap_btc(slate : &mut SwapSlate, stream : &mut TcpStream, rng : &mut OsRng, curve : &Secp256k1<All>) -> Result<SwapSlate, &'static str> {

    // Sender keypair
    let ssk = create_private_key(rng);
    let spk = ssk.public_key(curve);

    // witness / statement pair
    let x = create_private_key(rng);
    let X = x.public_key(curve);

    Err("Not implemented")
}