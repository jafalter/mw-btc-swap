use crate::{bitcoin::btcroutines::{create_private_key, deserialize_priv_key, deserialize_pub_key, serialize_priv_key, serialize_pub_key}, grin::grin_routines::{deserialize_grin_pub_key, grin_pk_from_btc_pk, grin_sk_from_btc_sk}};
use crate::net::tcp::write_to_stream;
use crate::SwapSlate;
use crate::{
    bitcoin::{
        bitcoin_core::BitcoinCore,
        btcroutines::{create_lock_transaction, get_lock_pub_script},
    },
    constants::{BTC_FEE, MAX_ATTEMPTS_VERF_FUNDS},
    grin::{grin_core::GrinCore, grin_tx::GrinTx},
    net::tcp::read_from_stream,
};
use bitcoin::secp256k1::All;
use bitcoin::util::key::PublicKey;
use bitcoin::util::psbt::serialize::Serialize;
use bitcoin::{secp256k1::Secp256k1, Address};
use grin_util::secp::Secp256k1 as GrinSecp256k1;
use rand::rngs::OsRng;
use std::convert::TryFrom;
use std::{net::TcpStream, thread, time::Duration};

/// Runs the mimblewimble side of the setup phase of the atomic swap
///
/// # Arguments
///
/// * `slate` reference to the atomic swap slate
pub fn setup_phase_swap_mw(
    slate: &mut SwapSlate,
    stream: &mut TcpStream,
    rng: &mut OsRng,
    secp: &Secp256k1<All>,
    grin_core: &mut GrinCore,
    btc_core: &mut BitcoinCore,
    grin_tx: &mut GrinTx,
) -> Result<(), &'static str> {
    println!("Starting setup phase MW");
    let mut msg_bob = "".to_string();

    // Receiver Bitcoin keypair
    let sk_a = create_private_key(rng);
    let pub_a = sk_a.public_key(secp);
    slate.prv_slate.btc.sk = Some(serialize_priv_key(&sk_a));
    slate.pub_slate.btc.pub_a = Some(serialize_pub_key(&pub_a));

    // Send public key to peer
    write_to_stream(stream, &hex::encode(pub_a.serialize()));

    // Bobs pubkey
    msg_bob = read_from_stream(stream);
    let pub_b =
        PublicKey::from_slice(&hex::decode(msg_bob).expect("Failed to decode senders pubkey"))
            .expect("Failed to deserialize senders pubkey");
    slate.pub_slate.btc.pub_b = Some(serialize_pub_key(&pub_b));

    // Statement x
    msg_bob = read_from_stream(stream);
    let pub_x = PublicKey::from_slice(&hex::decode(msg_bob).expect("Failed to decode statement X"))
        .expect("Failed to deserialize statement X");
    slate.pub_slate.btc.pub_x = Some(serialize_pub_key(&pub_x));

    // Bitcoin lock height
    msg_bob = read_from_stream(stream);
    let lock_time_btc: i64 = msg_bob.parse::<i64>().unwrap();
    // We now calcualte the bitcoin address on which Bob is supposed to lock his BTC
    let pub_script = get_lock_pub_script(pub_a, pub_x, pub_b, lock_time_btc, true);
    let addr = Address::from_script(&pub_script, bitcoin::Network::Testnet).unwrap();
    // index the address on our Bitcoin Core node
    btc_core.import_btc_address(addr.clone()).unwrap();

    // Now wait for Bob to send the lock address himself and then verify the locked funds
    msg_bob = read_from_stream(stream);
    if addr.clone().to_string() != msg_bob {
        Err("Lock address sent by Bob doesn't match what we have calculated, stopping swap")
    } else {
        let mut verified_funds = false;
        let mut attempts = 0;
        while !verified_funds && attempts < MAX_ATTEMPTS_VERF_FUNDS {
            let balance = btc_core
                .get_address_final_balance_addr(addr.clone())
                .unwrap_or(0);
            if balance > 0 {
                if balance != slate.pub_slate.btc.amount {
                    println!("Balance on address does not match what was expected");
                    attempts = MAX_ATTEMPTS_VERF_FUNDS;
                } else {
                    verified_funds = true;
                }
            } else {
                attempts = attempts + 1;
                println!("Failed to verify funds locked on btc side, trying again in 1 minute...");
                thread::sleep(Duration::from_secs(60));
            }
        }

        if !verified_funds {
            Err("Faild to verify that btc funds are correctly locked")
        } else {
            let grin_height = grin_core.get_block_height().unwrap();
            let grin_lock_height = grin_height + slate.pub_slate.mw.timelock;
            // Send over grin_lock_height to Bob
            write_to_stream(stream, &grin_lock_height.to_string());

            // Create shared MW output
            let shared_out_result = grin_tx.dshared_out_mw_tx_alice(
                slate.prv_slate.mw.inputs.clone(),
                slate.pub_slate.mw.amount,
                0,
                stream,
            ).expect("Failed to run shared out protocol on Alice side");
            slate.prv_slate.mw.shared_coin = Some(shared_out_result.shared_coin.clone());

            // Timelocked transaction spending back to Alice
            let refund_result = grin_tx.dshared_inp_mw_tx_alice(
                shared_out_result.shared_coin.clone(), 
                shared_out_result.shared_coin.clone().value, 
                grin_lock_height, 
                stream).expect("Failed to run shared inp protocol on Alice side");

            // publish the two transactions
            grin_core.push_transaction(shared_out_result.tx.tx.unwrap())
                .expect("Failed to publish MW funding transaction");
            grin_core.push_transaction(refund_result.tx.tx.unwrap())
                .expect("Failed to publish MW refund transaction");

            Ok(())
        }
    }
}

/// Runs the bitcoin side of the setup phase of the atomic swap
///
/// # Arguments
///
/// * `slate` reference to the atomic swap slate
pub fn setup_phase_swap_btc(
    slate: &mut SwapSlate,
    stream: &mut TcpStream,
    rng: &mut OsRng,
    secp: &Secp256k1<All>,
    grin_core: &mut GrinCore,
    btc_core: &mut BitcoinCore,
    grin_tx: &mut GrinTx,
) -> Result<(), String> {
    println!("Starting setup phase BTC");
    let mut msg_alice = "".to_string();

    // Sender Bitcoin keypair (for the change address)
    let sk_b = create_private_key(rng);
    let pub_b = sk_b.public_key(secp);
    slate.pub_slate.btc.pub_b = Some(serialize_pub_key(&pub_b));
    slate.prv_slate.btc.sk = Some(serialize_priv_key(&sk_b));

    // Sender refund address
    let sk_r = create_private_key(rng);
    let pub_r = sk_r.public_key(secp);
    slate.prv_slate.btc.r_sk = Some(serialize_priv_key(&sk_r));

    // witness / statement pair
    let x = create_private_key(rng);
    let pub_x = x.public_key(secp);
    slate.prv_slate.btc.x = Some(serialize_priv_key(&x));
    slate.pub_slate.btc.pub_x = Some(serialize_pub_key(&pub_x));

    // get the receivers pub key
    msg_alice = read_from_stream(stream);
    let pub_a = PublicKey::from_slice(&hex::decode(msg_alice).unwrap())
        .expect("Alice sent and invalid pubkey");
    slate.pub_slate.btc.pub_a = Some(serialize_pub_key(&pub_a));

    // Send sender pub key and statement pub_x
    write_to_stream(stream, &hex::encode(pub_b.serialize()));
    write_to_stream(stream, &hex::encode(pub_x.serialize()));

    // Now we lock up those bitcoins
    let btc_current_height = btc_core.get_current_block_height()?;
    let inputs = slate.prv_slate.btc.inputs.clone();
    let btc_amount = slate.pub_slate.btc.amount;
    let btc_lock_height: i64 =
        i64::try_from(btc_current_height + slate.pub_slate.btc.timelock).unwrap();
    // Send the bitcoin locktime to alice
    write_to_stream(stream, &btc_lock_height.to_string());

    let tx_lock = create_lock_transaction(
        pub_a,
        pub_x,
        pub_b,
        pub_r,
        inputs,
        btc_amount,
        BTC_FEE,
        btc_lock_height,
    )?;
    let pub_script = tx_lock.output.get(0).unwrap().script_pubkey.clone();
    let address = Address::from_script(&pub_script, bitcoin::Network::Testnet).unwrap();

    btc_core
        .send_raw_transaction(tx_lock)
        .expect("Failed to send bitcoin lock transaction");

    // Send the address over to Alice and let her verify the locked funds
    write_to_stream(stream, &address.to_string());

    // Receive the grin side lock height from alice
    msg_alice = read_from_stream(stream);
    let lock_height_grin = msg_alice.parse::<i64>().unwrap();

    let shared_out_result = grin_tx.dshared_out_mw_tx_bob(slate.pub_slate.mw.amount, stream)?;
    slate.prv_slate.mw.shared_coin = Some(shared_out_result.shared_coin.clone());
    let shared_inp_result = grin_tx.dshared_inp_mw_tx_bob(
        shared_out_result.shared_coin.clone(), 
        shared_out_result.shared_coin.clone().value, 
        u64::try_from(lock_height_grin).unwrap(), 
        stream)?;

    Ok(())
}

pub fn exec_phase_swap_mw(
    slate: &mut SwapSlate,
    stream: &mut TcpStream,
    btc_core: &mut BitcoinCore,
    grin_tx: &mut GrinTx,
    secp: &GrinSecp256k1,
) -> Result<(), String> {
    let shared_coin = slate.prv_slate.mw.shared_coin.clone().unwrap();
    let pub_x = deserialize_pub_key(&slate.pub_slate.btc.pub_x.unwrap());
    let pub_x_grin = grin_pk_from_btc_pk(&pub_x, secp);
    let result = grin_tx.dcontract_mw_tx_alice(shared_coin, shared_coin.value, 0, pub_x_grin, stream)?;
    
    Err(String::from("Not implemented"))
}

pub fn exec_phase_swap_btc(
    slate: &mut SwapSlate,
    stream: &mut TcpStream,
    btc_core: &mut BitcoinCore,
    grin_tx: &mut GrinTx,
    secp: &GrinSecp256k1,
) -> Result<(), String> {
    let shared_coin = slate.prv_slate.mw.shared_coin.clone().unwrap();
    let x = deserialize_priv_key(&slate.prv_slate.btc.x.unwrap());
    let x_grin = grin_sk_from_btc_sk(&x, secp);
    grin_tx.dcontract_mw_tx_bob(shared_coin, shared_coin.value, 0, x_grin, stream)?;

    Err(String::from("Not implemented"))
}
