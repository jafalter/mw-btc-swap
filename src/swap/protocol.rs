use crate::{bitcoin::{bitcoin_types::BTCInput, btcroutines::{create_private_key, create_spend_lock_transaction, deserialize_priv_key, deserialize_pub_key, deserialize_script, private_key_from_grin_sk, serialize_priv_key, serialize_pub_key, sign_lock_transaction_redeemer, sign_lock_transaction_refund, sign_p2pkh_transaction}}, grin::grin_routines::{deserialize_grin_pub_key, deserialize_secret_key, grin_pk_from_btc_pk, grin_sk_from_btc_sk}};
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
/// * `stream` TCP Channel
/// * `rng` Randomness generator
/// * `secp` Secp256k1 functions
/// * `grin_core` Grin core functions
/// * `btc_core` Bitcoin core functions
/// * `grin_tx` Grin transaction functions
pub fn setup_phase_swap_mw(
    slate: &mut SwapSlate,
    stream: &mut TcpStream,
    rng: &mut OsRng,
    secp: &Secp256k1<All>,
    grin_core: &mut GrinCore,
    btc_core: &mut BitcoinCore,
    grin_tx: &mut GrinTx,
) -> Result<(), String> {
    println!("Starting setup phase MW");
    slate.pub_slate.status = crate::enums::SwapStatus::SETUP;
    let mut msg_bob = "".to_string();

    println!("Creating and exchanging keys...");

    // Receiver Bitcoin keypair
    let sk_a = create_private_key(rng);
    let pub_a = sk_a.public_key(secp);
    slate.prv_slate.btc.sk = Some(serialize_priv_key(&sk_a));
    slate.pub_slate.btc.pub_a = Some(serialize_pub_key(&pub_a));

    // Send public key to peer
    write_to_stream(stream, &serialize_pub_key(&pub_a));

    // Bobs pubkey
    msg_bob = read_from_stream(stream);
    let pub_b = deserialize_pub_key(&msg_bob);
    slate.pub_slate.btc.pub_b = Some(serialize_pub_key(&pub_b));

    // Statement x
    msg_bob = read_from_stream(stream);
    let pub_x = deserialize_pub_key(&msg_bob);
    slate.pub_slate.btc.pub_x = Some(serialize_pub_key(&pub_x));

    // Bitcoin lock height
    msg_bob = read_from_stream(stream);
    let lock_time_btc: i64 = msg_bob.parse::<i64>().unwrap();
    slate.pub_slate.btc.lock_time = Some(lock_time_btc);
    // We now calculate the bitcoin address on which Bob is supposed to lock his BTC
    let pub_script = get_lock_pub_script(pub_a, pub_x, pub_b, lock_time_btc, true);
    let addr = Address::from_script(&pub_script, bitcoin::Network::Testnet).unwrap();
    // index the address on our Bitcoin Core node
    btc_core.import_btc_address(addr.clone()).unwrap();

    // Now wait for Bob to send the lock address himself and then verify the locked funds
    let address = read_from_stream(stream);
    let txid = read_from_stream(stream);
    println!("Verifing the locked funds on address : {} and txid: {}", address, txid);
    if addr.clone().to_string() != msg_bob {
        slate.pub_slate.status = crate::enums::SwapStatus::FAILED;
        Err(String::from("Lock address sent by Bob doesn't match what we have calculated, stopping swap"))
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
                println!("Failed to verify funds locked, trying again in 1 minute...");
                thread::sleep(Duration::from_secs(60));
            }
        }

        if !verified_funds {
            slate.pub_slate.status = crate::enums::SwapStatus::FAILED;
            Err(String::from("Failed to verify that btc funds are correctly locked"))
        } else {
            println!("Successfully verified the locked funds!");
            slate.pub_slate.btc.lock = Some(BTCInput::new2(txid, 0, slate.pub_slate.btc.amount, sk_a, pub_a, pub_script));
            let grin_height = grin_core.get_block_height().unwrap();
            let grin_lock_height = grin_height + slate.pub_slate.mw.timelock;
            slate.pub_slate.mw.lock_time = Some(i64::try_from(grin_lock_height).unwrap());
            // Send over grin_lock_height to Bob
            write_to_stream(stream, &grin_lock_height.to_string());

            // Create shared MW output
            println!("Running protocol to create shared Mimblewimble output...");
            let shared_out_result = grin_tx.dshared_out_mw_tx_alice(
                slate.prv_slate.mw.inputs.clone(),
                slate.pub_slate.mw.amount,
                0,
                stream,
            )?;
            slate.prv_slate.mw.shared_coin = Some(shared_out_result.shared_coin.clone());
            slate.prv_slate.mw.change_coin = shared_out_result.change_coin.clone();
            println!("Mimblewimble change coin: {}", slate.prv_slate.mw.change_coin.clone().unwrap().to_string());

            // Timelocked transaction spending back to Alice
            println!("Running protocol to spend shared Mimblewimble output back as a timelocked refund...");
            let refund_result = grin_tx.dshared_inp_mw_tx_alice(
                shared_out_result.shared_coin.clone(), 
                shared_out_result.shared_coin.clone().value, 
                grin_lock_height, 
                stream)?;
            slate.prv_slate.mw.refund_coin = Some(refund_result.coin);

            // publish the two transactions
            grin_core.push_transaction(shared_out_result.tx.tx.unwrap())?;
            grin_core.push_transaction(refund_result.tx.tx.unwrap())?;

            println!("Successfully finished setup protocol on Mimblewimble side");

            Ok(())
        }
    }
}

/// Runs the bitcoin side of the setup phase of the atomic swap
///
/// # Arguments
///
/// * `slate` reference to the atomic swap slate
/// * `stream` TCP channel
/// * `rng` Randomness generator
/// * `secp` Secp256k1 functions
/// * `grin_core` Grin core functions
/// * `btc_core` Bitcoin core functions
/// * `grin_tx` Grin transaction functions
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
    slate.pub_slate.status = crate::enums::SwapStatus::SETUP;
    let mut msg_alice = "".to_string();


    println!("Exchanging keys...");
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
    let pub_a = deserialize_pub_key(&msg_alice);
    slate.pub_slate.btc.pub_a = Some(serialize_pub_key(&pub_a));

    // Send sender pub key and statement pub_x
    write_to_stream(stream, &serialize_pub_key(&pub_b));
    write_to_stream(stream, &serialize_pub_key(&pub_x));

    // Now we lock up those bitcoins
    println!("Building Bitcoin lock transaction...");
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
        inputs.clone(),
        btc_amount,
        BTC_FEE,
        btc_lock_height,
    )?;
    let tx_lock_clone = tx_lock.clone();
    let pub_script = tx_lock_clone.output.get(0).unwrap().script_pubkey.clone();
    let ch_out = tx_lock_clone.output.get(1).unwrap();
    let address = Address::from_script(&pub_script, bitcoin::Network::Testnet).unwrap();

    let txid = tx_lock_clone.txid().to_string();
    let inp = inputs.get(0).unwrap();
    let inp_pub_script = deserialize_script(&inp.pub_script);
    let inp_sk = deserialize_priv_key(&inp.secret);
    let inp_pk = PublicKey::from_private_key(secp, &inp_sk);
    let signed_tx = sign_p2pkh_transaction(tx_lock, vec![inp_pub_script], vec![inp_sk], vec![inp_pk], secp);
    btc_core
        .send_raw_transaction(signed_tx.clone())?;
    let change = BTCInput::new2(txid.clone(), 1, ch_out.value, sk_r, pub_r, ch_out.script_pubkey.clone());
    slate.prv_slate.btc.change = Some(change.clone());
    println!("Published Bitcoin lock transaction with txid: {}, address: {}", txid, address.to_string());
    println!("Bitcoin change output: {}", change.clone().to_string());

    // Send the address, txid over to Alice and let her verify the locked funds
    write_to_stream(stream, &address.to_string());
    write_to_stream(stream, &txid);
    slate.pub_slate.btc.lock = Some(BTCInput::new2(txid, 0, btc_amount,  sk_b, pub_b, pub_script));

    // Receive the grin side lock height from alice
    msg_alice = read_from_stream(stream);
    let lock_height_grin = msg_alice.parse::<i64>().unwrap();
    slate.pub_slate.mw.lock_time = Some(lock_height_grin);

    println!("Running protocol to create shared Mimblewimble output...");
    let shared_out_result = grin_tx.dshared_out_mw_tx_bob(slate.pub_slate.mw.amount, stream)?;
    slate.prv_slate.mw.shared_coin = Some(shared_out_result.shared_coin.clone());
    println!("Running protocol to refund shared Mimblewimble output...");
    let shared_inp_result = grin_tx.dshared_inp_mw_tx_bob(
        shared_out_result.shared_coin.clone(), 
        shared_out_result.shared_coin.clone().value, 
        u64::try_from(lock_height_grin).unwrap(), 
        stream)?;

    println!("Successfully finished Setup protocol on Bitcoin side");
    Ok(())
}

/// Executes the Atomic Swap on Mimblewimble (owining) side
/// This requires that before the setup phase has been run already
///
/// # Arguments
///
/// * `slate` The swap slate, needs to be setup
/// * `stream` TCP channel
/// * `btc_core` Bitcoin core functions
/// * `rng` Randomness generator
/// * `grin_tx` Grin transaction functions
/// * `grin_secp` Grin version of Secp256k1 functions
/// * `btc_secp` Bitcoin version of Secp256k1 functions
pub fn exec_phase_swap_mw(
    slate: &mut SwapSlate,
    stream: &mut TcpStream,
    btc_core: &mut BitcoinCore,
    rng: &mut OsRng,
    grin_tx: &mut GrinTx,
    grin_secp: &GrinSecp256k1,
    btc_secp: &Secp256k1<All>,
) -> Result<(), String> {
    slate.pub_slate.status = crate::enums::SwapStatus::EXECUTING;
    println!("Running Atomic Swap execution phase on mimblewimble side");
    let shared_coin = slate.prv_slate.mw.shared_coin.clone().unwrap();
    let value = shared_coin.value;
    let pub_x = deserialize_pub_key(&slate.pub_slate.btc.pub_x.clone().unwrap());
    let pub_x_grin = grin_pk_from_btc_pk(&pub_x, grin_secp);

    println!("Running Mimblewimble Contract transaction protocol");
    let result = grin_tx.dcontract_mw_tx_alice(shared_coin, value, 0, pub_x_grin, stream)?;
    
    let sk_a2 = create_private_key(rng);
    let pub_a2 = PublicKey::from_private_key(btc_secp, &sk_a2);

    let pub_a = deserialize_pub_key(&slate.pub_slate.btc.pub_a.clone().unwrap());
    let pub_b = deserialize_pub_key(&slate.pub_slate.btc.pub_b.clone().unwrap());
    let pub_x = deserialize_pub_key(&slate.pub_slate.btc.pub_x.clone().unwrap());

    let x_btc = private_key_from_grin_sk(&result.x);

    let sk_a = deserialize_priv_key(&slate.prv_slate.btc.sk.clone().unwrap());

    println!("Creating Bitcoin redeem transaction");
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
    println!("Successfully completed Atomic Swap on Mimblewimble side");
    slate.pub_slate.status = crate::enums::SwapStatus::FINISHED;

    Ok(())
}

/// Executing the Atomic Swap on Bitcoin (owning) side
/// Running this protocol requires that before the setup protocol was successfully run
/// 
/// # Arguments
///
/// * `slate` the Atomic swap state, needs to be setup
/// * `stream` TCP channel
/// * `btc_core` Bitcoin core functions
/// * `grin_core` Grin core functions
/// * `grin_tx` Grin Transaction functions
/// * `secp` Grin version of Secp256k1 functionality
pub fn exec_phase_swap_btc(
    slate: &mut SwapSlate,
    stream: &mut TcpStream,
    btc_core: &mut BitcoinCore,
    grin_core: &mut GrinCore,
    grin_tx: &mut GrinTx,
    secp: &GrinSecp256k1,
) -> Result<(), String> {
    slate.pub_slate.status = crate::enums::SwapStatus::EXECUTING;
    let shared_coin = slate.prv_slate.mw.shared_coin.clone().unwrap();
    let value = shared_coin.value;
    let x = deserialize_priv_key(&slate.prv_slate.btc.x.clone().unwrap());
    let x_grin = grin_sk_from_btc_sk(&x, secp);
    let result = grin_tx.dcontract_mw_tx_bob(shared_coin, value, 0, x_grin, stream)?;
    slate.prv_slate.mw.swapped_coin = Some(result.coin);
    grin_core.push_transaction(result.tx.tx.unwrap())?;
    slate.pub_slate.status = crate::enums::SwapStatus::FINISHED;

    Ok(())
}
