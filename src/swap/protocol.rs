use crate::{bitcoin::{bitcoin_types::BTCInput, btcroutines::{create_private_key, create_spend_lock_transaction, deserialize_priv_key, deserialize_pub_key, private_key_from_grin_sk, serialize_priv_key, serialize_pub_key, sign_lock_transaction_redeemer}}, grin::grin_routines::{deserialize_grin_pub_key, deserialize_secret_key, grin_pk_from_btc_pk, grin_sk_from_btc_sk}};
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
use bitcoin::{PrivateKey, secp256k1::All};
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
    slate.pub_slate.btc.lock_time = Some(lock_time_btc);
    // We now calcualte the bitcoin address on which Bob is supposed to lock his BTC
    let pub_script = get_lock_pub_script(pub_a, pub_x, pub_b, lock_time_btc, true);
    let addr = Address::from_script(&pub_script, bitcoin::Network::Testnet).unwrap();
    // index the address on our Bitcoin Core node
    btc_core.import_btc_address(addr.clone()).unwrap();

    // Now wait for Bob to send the lock address himself and then verify the locked funds
    let address = read_from_stream(stream);
    let txid = read_from_stream(stream);
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
            slate.pub_slate.btc.lock = Some(BTCInput::new2(txid, 0, slate.pub_slate.btc.amount, sk_a, pub_a, pub_script));
            let grin_height = grin_core.get_block_height().unwrap();
            let grin_lock_height = grin_height + slate.pub_slate.mw.timelock;
            slate.pub_slate.mw.lock_time = Some(i64::try_from(grin_lock_height).unwrap());
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
            slate.prv_slate.mw.refund_coin = Some(refund_result.coin);

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
    slate.pub_slate.btc.lock_time = Some(btc_lock_height);

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

    let txid = tx_lock.clone().txid().to_string();
    btc_core
        .send_raw_transaction(tx_lock)?;

    // Send the address, txid over to Alice and let her verify the locked funds
    write_to_stream(stream, &address.to_string());
    write_to_stream(stream, &txid);
    slate.pub_slate.btc.lock = Some(BTCInput::new2(txid, 0, btc_amount,  sk_b, pub_b, pub_script));

    // Receive the grin side lock height from alice
    msg_alice = read_from_stream(stream);
    let lock_height_grin = msg_alice.parse::<i64>().unwrap();
    slate.pub_slate.mw.lock_time = Some(lock_height_grin);

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
    rng: &mut OsRng,
    grin_tx: &mut GrinTx,
    grin_secp: &GrinSecp256k1,
    btc_secp: &Secp256k1<All>,
) -> Result<(), String> {
    let shared_coin = slate.prv_slate.mw.shared_coin.clone().unwrap();
    let value = shared_coin.value;
    let pub_x = deserialize_pub_key(&slate.pub_slate.btc.pub_x.clone().unwrap());
    let pub_x_grin = grin_pk_from_btc_pk(&pub_x, grin_secp);
    let result = grin_tx.dcontract_mw_tx_alice(shared_coin, value, 0, pub_x_grin, stream)?;
    
    let sk_a2 = create_private_key(rng);
    let pub_a2 = PublicKey::from_private_key(btc_secp, &sk_a2);

    let pub_a = deserialize_pub_key(&slate.pub_slate.btc.pub_a.clone().unwrap());
    let pub_b = deserialize_pub_key(&slate.pub_slate.btc.pub_b.clone().unwrap());
    let pub_x = deserialize_pub_key(&slate.pub_slate.btc.pub_x.clone().unwrap());

    let x_btc = private_key_from_grin_sk(&result.x);

    let sk_a = deserialize_priv_key(&slate.prv_slate.btc.sk.clone().unwrap());

    // Now we can spent the Bitcoin
    let redeem_tx = create_spend_lock_transaction(&pub_a2, slate.pub_slate.btc.lock.clone().unwrap(), slate.pub_slate.btc.amount, BTC_FEE, 0)?;
    let lock_script = get_lock_pub_script(pub_a, pub_x, pub_b, slate.pub_slate.btc.lock_time.unwrap(), false);
    let signed_redeem_tx = sign_lock_transaction_redeemer(redeem_tx, 0, lock_script, sk_a, x_btc, btc_secp);
    let o = signed_redeem_tx.output.get(0).unwrap();
    let txid = signed_redeem_tx.txid().to_string();
    btc_core.send_raw_transaction(signed_redeem_tx.clone())?;
    slate.prv_slate.btc.swapped = Some(BTCInput::new2(txid, 
    0, 
    o.value, 
    sk_a2, 
    pub_a2, 
    o.script_pubkey.clone()));

    Ok(())
}

pub fn exec_phase_swap_btc(
    slate: &mut SwapSlate,
    stream: &mut TcpStream,
    btc_core: &mut BitcoinCore,
    grin_core: &mut GrinCore,
    grin_tx: &mut GrinTx,
    secp: &GrinSecp256k1,
) -> Result<(), String> {
    let shared_coin = slate.prv_slate.mw.shared_coin.clone().unwrap();
    let value = shared_coin.value;
    let x = deserialize_priv_key(&slate.prv_slate.btc.x.clone().unwrap());
    let x_grin = grin_sk_from_btc_sk(&x, secp);
    let result = grin_tx.dcontract_mw_tx_bob(shared_coin, value, 0, x_grin, stream)?;
    slate.prv_slate.mw.swapped_coin = Some(result.coin);
    grin_core.push_transaction(result.tx.tx.unwrap())?;

    Ok(())
}
