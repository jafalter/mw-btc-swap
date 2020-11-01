use crate::net::tcp::read_from_stream;
use crate::net::tcp::write_to_stream;
use crate::bitcoin::btcroutines::create_private_key;
use rand::rngs::OsRng;
use std::net::TcpStream;
use crate::SwapSlate;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::All;
use bitcoin::util::key::PublicKey;
use bitcoin::util::psbt::serialize::Serialize;

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

    // Senders pubkey
    let hexspk = read_from_stream(stream);
    let spk = PublicKey::from_slice(
        &hex::decode(hexspk)
            .expect("Failed to decode senders pubkey")
    ).expect("Failed to deserialize senders pubkey");

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

    // get the receivers pub key
    let hexrpk = read_from_stream(stream);
    let rpk = PublicKey::from_slice(
        &hex::decode(hexrpk)
        .expect("Failed to decode receivers pubkey")
    ).expect("Failed to deserialize receivers pubkey");

    // Send sender pub key and statement
    write_to_stream(stream, &hex::encode(spk.serialize()));
    write_to_stream(stream, &hex::encode(X.serialize()));

    Err("Not implemented")
}