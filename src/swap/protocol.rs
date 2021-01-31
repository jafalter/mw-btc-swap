use crate::{bitcoin::{bitcoin_core::BitcoinCore, btcroutines::create_lock_transaction}, constants::BTC_FEE, grin::grin_core::GrinCore, net::tcp::read_from_stream};
use crate::net::tcp::write_to_stream;
use crate::bitcoin::btcroutines::create_private_key;
use rand::rngs::OsRng;
use std::net::TcpStream;
use crate::SwapSlate;
use bitcoin::{Address, secp256k1::Secp256k1};
use bitcoin::secp256k1::All;
use bitcoin::util::key::PublicKey;
use bitcoin::util::psbt::serialize::Serialize;
use std::convert::TryFrom;

/// Runs the mimblewimble side of the setup phase of the atomic swap
/// 
/// # Arguments
/// 
/// * `slate` reference to the atomic swap slate
pub fn setup_phase_swap_mw(slate : &mut SwapSlate, stream : &mut TcpStream, rng : &mut OsRng, secp : &Secp256k1<All>, grin_core : &GrinCore, btc_core : &BitcoinCore) -> Result<SwapSlate, &'static str> {
    println!("Starting setup phase MW");
    let mut msg_bob = "".to_string();

    // Receiver Bitcoin keypair
    let sk_a = create_private_key(rng);
    let pub_a = sk_a.public_key(secp);

    // Send public key to peer
    write_to_stream(stream, &hex::encode(pub_a.serialize()));

    // Senders pubkey
    msg_bob = read_from_stream(stream);
    let pub_b = PublicKey::from_slice(
        &hex::decode(msg_bob)
            .expect("Failed to decode senders pubkey")
    ).expect("Failed to deserialize senders pubkey");

    // Statement x
    msg_bob = read_from_stream(stream);
    let pub_x = PublicKey::from_slice(
        &hex::decode(msg_bob)
            .expect("Failed to decode statement X")
    ).expect("Failed to deserialize statement X");

    Err("Not implemented")
}

/// Runs the bitcoin side of the setup phase of the atomic swap
/// 
/// # Arguments
/// 
/// * `slate` reference to the atomic swap slate
pub fn setup_phase_swap_btc(slate : &mut SwapSlate, stream : &mut TcpStream, rng : &mut OsRng, secp : &Secp256k1<All>, grin_core : &GrinCore, btc_core : &BitcoinCore) -> Result<SwapSlate, String> {
    println!("Starting setup phase BTC");
    let mut msg_alice = "".to_string();

    // Sender Bitcoin keypair (for the change address)
    let sk_b = create_private_key(rng);
    let pub_b = sk_b.public_key(secp);

    // Sender refund address
    let sk_r = create_private_key(rng);
    let pub_r = sk_r.public_key(secp);

    // witness / statement pair
    let x = create_private_key(rng);
    let pub_x = x.public_key(secp);

    // get the receivers pub key
    msg_alice = read_from_stream(stream);
    let pub_a = PublicKey::from_slice(
        &hex::decode(msg_alice).unwrap()
    ).expect("Alice sent and invalid pubkey");

    // Send sender pub key and statement
    write_to_stream(stream, &hex::encode(pub_b.serialize()));
    write_to_stream(stream, &hex::encode(pub_x.serialize()));

    // Now we lock up those bitcoins
    let btc_current_height = btc_core.get_current_block_height()?;
    let inputs = slate.prv_slate.btc.inputs.clone();
    let btc_amount = slate.pub_slate.btc.amount;
    let btc_lock_height :i64 = i64::try_from(btc_current_height + slate.pub_slate.btc.timelock)
        .unwrap();
    let tx_lock = create_lock_transaction(
        pub_a, 
        pub_x, 
        pub_b, 
        pub_r, 
        inputs, 
        btc_amount, 
        BTC_FEE, 
       btc_lock_height
    )?;
    let pub_script = tx_lock.output.get(0).unwrap().script_pubkey;
    let address = Address::from_script(&pub_script, bitcoin::Network::Testnet);

    btc_core.send_raw_transaction(tx_lock)
        .expect("Failed to send bitcoin lock transaction");

    // Send the transaction over to Alice such that she can verify its validity

    Err(String::from("Not implemented"))
}