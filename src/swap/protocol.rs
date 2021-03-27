use crate::net::tcp::send_msg;
use crate::SwapSlate;
use crate::{
    bitcoin::{
        bitcoin_core::BitcoinCore,
        btcroutines::{create_lock_transaction, get_lock_pub_script},
    },
    constants::{BTC_FEE, MAX_ATTEMPTS_VERF_FUNDS},
    grin::{grin_core::GrinCore, grin_tx::GrinTx},
    net::tcp::receive_msg,
};
use crate::{
    bitcoin::{
        bitcoin_types::BTCInput,
        btcroutines::{
            create_private_key, create_spend_lock_transaction, deserialize_priv_key,
            deserialize_pub_key, deserialize_script, private_key_from_grin_sk, serialize_priv_key,
            serialize_pub_key, serialize_script, sign_lock_transaction_redeemer,
            sign_lock_transaction_refund, sign_p2pkh_transaction,
        },
    },
    constants::{BTC_BLOCK_TIME, GRIN_BLOCK_TIME},
    grin::grin_routines::{
        deserialize_grin_pub_key, deserialize_secret_key, estimate_fees, grin_pk_from_btc_pk,
        grin_sk_from_btc_sk,
    },
};
use bitcoin::util::key::PublicKey;
use bitcoin::util::psbt::serialize::Serialize;
use bitcoin::{secp256k1::All, PrivateKey};
use bitcoin::{secp256k1::Secp256k1, Address};
use grin_core::global::set_local_chain_type;
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
pub fn locking_phase_swap_mw(
    slate: &mut SwapSlate,
    stream: &mut TcpStream,
    rng: &mut OsRng,
    secp: &Secp256k1<All>,
    grin_core: &mut GrinCore,
    btc_core: &mut BitcoinCore,
    grin_tx: &mut GrinTx,
) -> Result<(), String> {
    set_local_chain_type(grin_core::global::ChainTypes::Testnet);
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
    send_msg(stream, &serialize_pub_key(&pub_a));

    // Bobs pubkey
    msg_bob = receive_msg(stream);
    let pub_b = deserialize_pub_key(&msg_bob);
    slate.pub_slate.btc.pub_b = Some(serialize_pub_key(&pub_b));

    // Statement x
    msg_bob = receive_msg(stream);
    let pub_x = deserialize_pub_key(&msg_bob);
    slate.pub_slate.btc.pub_x = Some(serialize_pub_key(&pub_x));

    // Bitcoin lock height
    msg_bob = receive_msg(stream);
    let lock_time_btc: i64 = msg_bob.parse::<i64>().unwrap();
    slate.pub_slate.btc.lock_time = Some(lock_time_btc);
    // We now calculate the bitcoin address on which Bob is supposed to lock his BTC
    let pub_script = get_lock_pub_script(pub_a, pub_x, pub_b, lock_time_btc, true);
    let addr = Address::from_script(&pub_script, bitcoin::Network::Testnet).unwrap();
    // index the address on our Bitcoin Core node
    btc_core.import_btc_address(addr.clone()).unwrap();

    // Now wait for Bob to send the lock address himself and then verify the locked funds
    let address = receive_msg(stream);
    let txid = receive_msg(stream);
    println!(
        "Verifing the locked funds on address : {} and txid: {}",
        address, txid
    );
    if addr.clone().to_string() != address {
        slate.pub_slate.status = crate::enums::SwapStatus::FAILED;
        Err(String::from(
            "Lock address sent by Bob doesn't match what we have calculated, stopping swap",
        ))
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
            Err(String::from(
                "Failed to verify that btc funds are correctly locked",
            ))
        } else {
            println!("Successfully verified the locked funds!");
            slate.prv_slate.btc.lock = Some(BTCInput::new2(
                txid,
                0,
                slate.pub_slate.btc.amount,
                sk_a,
                pub_a,
                pub_script,
            ));
            let grin_height = grin_core.get_block_height().unwrap();
            let grin_lock_height = grin_height + slate.pub_slate.mw.timelock;
            slate.pub_slate.mw.lock_time = Some(i64::try_from(grin_lock_height).unwrap());
            // Send over grin_lock_height to Bob
            send_msg(stream, &grin_lock_height.to_string());

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
            println!(
                "Mimblewimble change coin: {}",
                slate.prv_slate.mw.change_coin.clone().unwrap().to_string()
            );
            println!(
                "Share coin mw side: {}",
                shared_out_result.shared_coin.to_string()
            );
            let fee = estimate_fees(1, 1, 1);
            let fund_value = shared_out_result.shared_coin.clone().value - fee;
            println!("Refund tx fund value: {}", fund_value);

            // Timelocked transaction spending back to Alice
            println!("Running protocol to spend shared Mimblewimble output back as a timelocked refund...");
            let refund_result = grin_tx.dshared_inp_mw_tx_bob(
                shared_out_result.shared_coin.clone(),
                fund_value,
                grin_lock_height,
                stream,
            )?;
            slate.prv_slate.mw.refund_coin = refund_result.coin;
            slate.prv_slate.mw.refund_tx = refund_result.tx.tx.clone();

            // publish the funding transactions
            grin_core.push_transaction(shared_out_result.tx.tx.unwrap())?;

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
pub fn locking_phase_swap_btc(
    slate: &mut SwapSlate,
    stream: &mut TcpStream,
    rng: &mut OsRng,
    secp: &Secp256k1<All>,
    grin_core: &mut GrinCore,
    btc_core: &mut BitcoinCore,
    grin_tx: &mut GrinTx,
) -> Result<(), String> {
    set_local_chain_type(grin_core::global::ChainTypes::Testnet);
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
    msg_alice = receive_msg(stream);
    let pub_a = deserialize_pub_key(&msg_alice);
    slate.pub_slate.btc.pub_a = Some(serialize_pub_key(&pub_a));

    // Send sender pub key and statement pub_x
    send_msg(stream, &serialize_pub_key(&pub_b));
    send_msg(stream, &serialize_pub_key(&pub_x));

    // Now we lock up those bitcoins
    println!("Building Bitcoin lock transaction...");
    let btc_current_height = btc_core.get_current_block_height()?;
    let inputs = slate.prv_slate.btc.inputs.clone();
    let btc_amount = slate.pub_slate.btc.amount;
    let btc_lock_height: i64 =
        i64::try_from(btc_current_height + slate.pub_slate.btc.timelock).unwrap();
    // Send the bitcoin locktime to alice
    send_msg(stream, &btc_lock_height.to_string());
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

    let inp = inputs.get(0).unwrap();
    let inp_pub_script = deserialize_script(&inp.pub_script);
    let inp_sk = deserialize_priv_key(&inp.secret);
    let inp_pk = PublicKey::from_private_key(secp, &inp_sk);
    let signed_tx = sign_p2pkh_transaction(
        tx_lock,
        vec![inp_pub_script],
        vec![inp_sk],
        vec![inp_pk],
        secp,
    );
    let txid = signed_tx.txid().to_string();
    btc_core.send_raw_transaction(signed_tx.clone())?;
    let change = BTCInput::new2(
        txid.clone(),
        1,
        ch_out.value,
        sk_r,
        pub_r,
        ch_out.script_pubkey.clone(),
    );
    slate.prv_slate.btc.change = Some(change.clone());
    println!(
        "Published Bitcoin lock transaction with txid: {}, address: {}",
        txid,
        address.to_string()
    );
    println!("Bitcoin change output: {}", change.clone().to_string());

    // Send the address, txid over to Alice and let her verify the locked funds
    send_msg(stream, &address.to_string());
    send_msg(stream, &txid);
    slate.prv_slate.btc.lock = Some(BTCInput::new2(txid, 0, btc_amount, sk_b, pub_b, pub_script));

    // Receive the grin side lock height from alice
    msg_alice = receive_msg(stream);
    let lock_height_grin = msg_alice.parse::<i64>().unwrap();
    slate.pub_slate.mw.lock_time = Some(lock_height_grin);

    println!("Running protocol to create shared Mimblewimble output...");
    let shared_out_result = grin_tx.dshared_out_mw_tx_bob(slate.pub_slate.mw.amount, stream)?;
    slate.prv_slate.mw.shared_coin = Some(shared_out_result.shared_coin.clone());
    println!(
        "Shared coin on BTC side: {}",
        shared_out_result.shared_coin.to_string()
    );

    println!("Running protocol to refund shared Mimblewimble output...");
    let fee = estimate_fees(1, 1, 1);
    let fund_value = shared_out_result.shared_coin.clone().value - fee;
    println!("Fund value of refund transaction: {}", fund_value);
    let shared_inp_result = grin_tx.dshared_inp_mw_tx_alice(
        shared_out_result.shared_coin.clone(),
        fund_value,
        u64::try_from(lock_height_grin).unwrap(),
        stream,
    )?;

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
    grin_core: &mut GrinCore,
    grin_secp: &GrinSecp256k1,
    btc_secp: &Secp256k1<All>,
) -> Result<(), String> {
    if check_if_enough_time(grin_core, btc_core, slate) {
        set_local_chain_type(grin_core::global::ChainTypes::Testnet);
        slate.pub_slate.status = crate::enums::SwapStatus::EXECUTING;
        println!("Running Atomic Swap execution phase on mimblewimble side");
        let shared_coin = slate.prv_slate.mw.shared_coin.clone().unwrap();
        let value = shared_coin.value;
        let pub_x = deserialize_pub_key(&slate.pub_slate.btc.pub_x.clone().unwrap());
        let pub_x_grin = grin_pk_from_btc_pk(&pub_x, grin_secp);

        println!("Running Mimblewimble Contract transaction protocol");
        let fee = estimate_fees(1, 1, 1);
        let fund_value = value - fee;
        let result =
            grin_tx.dcontract_mw_tx_alice(shared_coin, fund_value, 0, pub_x_grin, stream)?;

        let sk_a2 = create_private_key(rng);
        let pub_a2 = PublicKey::from_private_key(btc_secp, &sk_a2);

        let pub_a = deserialize_pub_key(&slate.pub_slate.btc.pub_a.clone().unwrap());
        let pub_b = deserialize_pub_key(&slate.pub_slate.btc.pub_b.clone().unwrap());
        let pub_x = deserialize_pub_key(&slate.pub_slate.btc.pub_x.clone().unwrap());

        let x_btc = private_key_from_grin_sk(&result.x);
        println!("Extracted x value: {}", serialize_priv_key(&x_btc));

        let sk_a = deserialize_priv_key(&slate.prv_slate.btc.sk.clone().unwrap());

        println!("Creating Bitcoin redeem transaction");
        // Now we can spent the Bitcoin
        let redeem_tx = create_spend_lock_transaction(
            &pub_a2,
            slate.prv_slate.btc.lock.clone().unwrap(),
            slate.pub_slate.btc.amount,
            BTC_FEE,
            0,
        )?;
        let lock_script = get_lock_pub_script(
            pub_a,
            pub_x,
            pub_b,
            slate.pub_slate.btc.lock_time.unwrap(),
            false,
        );
        let signed_redeem_tx =
            sign_lock_transaction_redeemer(redeem_tx, 0, lock_script, sk_a, x_btc, btc_secp);
        let o = signed_redeem_tx.output.get(0).unwrap();
        let txid = signed_redeem_tx.txid().to_string();
        btc_core.send_raw_transaction(signed_redeem_tx.clone())?;
        slate.prv_slate.btc.swapped = Some(BTCInput::new2(
            txid,
            0,
            o.value,
            sk_a2,
            pub_a2,
            o.script_pubkey.clone(),
        ));
        println!("Successfully completed Atomic Swap on Mimblewimble side");
        slate.pub_slate.status = crate::enums::SwapStatus::FINISHED;

        Ok(())
    } else {
        Err(String::from("Not enough time left to execute atomic swap"))
    }
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
    set_local_chain_type(grin_core::global::ChainTypes::Testnet);
    if check_if_enough_time(grin_core, btc_core, slate) {
        slate.pub_slate.status = crate::enums::SwapStatus::EXECUTING;
        let shared_coin = slate.prv_slate.mw.shared_coin.clone().unwrap();
        let value = shared_coin.value;
        let x = deserialize_priv_key(&slate.prv_slate.btc.x.clone().unwrap());
        let x_grin = grin_sk_from_btc_sk(&x, secp);
        let fee = estimate_fees(1, 1, 1);
        let fund_value = value - fee;
        let result = grin_tx.dcontract_mw_tx_bob(shared_coin, fund_value, 0, x_grin, stream)?;
        slate.prv_slate.mw.swapped_coin = result.coin;
        grin_core.push_transaction(result.tx.tx.unwrap())?;
        slate.pub_slate.status = crate::enums::SwapStatus::FINISHED;
        Ok(())
    } else {
        Err(String::from("Not enough time left to execute atomic swap"))
    }
}

/// Refund coins to the original owner
/// Sends refund transaction to grin
///
/// # Arguments
///
/// * `slate` Atomic Swap Slate
/// * `btc_core` Bitcoin core node functionality
/// * `grin_core` Grin core node functionality
pub fn refund_phase_swap_mw(
    slate: &mut SwapSlate,
    btc_core: &mut BitcoinCore,
    grin_core: &mut GrinCore,
) -> Result<(), String> {
    if can_refund(grin_core, btc_core, slate) {
        let refund_tx = slate.prv_slate.mw.refund_tx.clone().unwrap();
        grin_core.push_transaction(refund_tx)?;
        slate.pub_slate.status = crate::enums::SwapStatus::FAILED;
        Ok(())
    } else {
        Err(String::from("Can't refund yet, too early"))
    }
}

/// Refund coins to the original owner
/// Sends transaction to bitcoin core to refund the locked bitcoin
///
/// # Arguments
///
/// * `slate` Atomic Swap Slate
/// * `btc_core` Bitcoin core node functionality
/// * `grin_core` Grin core node functionality
/// * `rng` randomness tape
pub fn refund_phase_swap_btc(
    slate: &mut SwapSlate,
    btc_core: &mut BitcoinCore,
    grin_core: &mut GrinCore,
    btc_secp: &Secp256k1<All>,
    rng: &mut OsRng,
) -> Result<(), String> {
    if can_refund(grin_core, btc_core, slate) {
        let sk = create_private_key(rng);
        let pk = PublicKey::from_private_key(btc_secp, &sk);

        let pub_a = deserialize_pub_key(&slate.pub_slate.btc.pub_a.clone().unwrap());
        let sk_b = deserialize_priv_key(&slate.prv_slate.btc.sk.clone().unwrap());
        let x = deserialize_priv_key(&slate.prv_slate.btc.x.clone().unwrap());
        let pub_b = PublicKey::from_private_key(btc_secp, &sk_b);
        let pub_x = PublicKey::from_private_key(btc_secp, &x);
        let lock_time = slate.pub_slate.btc.lock_time.unwrap();

        let refund_tx = create_spend_lock_transaction(
            &pk,
            slate.prv_slate.btc.lock.clone().unwrap(),
            slate.pub_slate.btc.amount,
            BTC_FEE,
            u32::try_from(lock_time).unwrap()
        )?;
        let lock_script = get_lock_pub_script(
            pub_a,
            pub_x,
            pub_b,
            lock_time,
            false,
        );

        let signed_tx = sign_lock_transaction_refund(refund_tx, 0, lock_script, sk_b, btc_secp);
        btc_core.send_raw_transaction(signed_tx.clone())?;
        let o = signed_tx.output.get(0).unwrap();
        slate.prv_slate.btc.refunded = Some(BTCInput {
            txid: signed_tx.clone().txid().to_string(),
            vout: 0,
            value: o.value,
            secret: serialize_priv_key(&sk),
            pub_key: serialize_pub_key(&pk),
            pub_script: serialize_script(&o.script_pubkey),
        });
        slate.pub_slate.status = crate::enums::SwapStatus::FAILED;

        Ok(())
    } else {
        Err(String::from("Can't refund yet, too early"))
    }
}

/// Query current block heights to see if enough time is left to complete the swap
/// Will return true if yes, false otherwise. Enough time is given if there
/// is at least 1 hour left in average block length
///
/// # Arguments
///
/// * `grin_core` Grin core node functions
/// * `btc_core` Bitcoin core node functions
/// * `slate` Swap slate
fn check_if_enough_time(
    grin_core: &mut GrinCore,
    btc_core: &mut BitcoinCore,
    slate: &SwapSlate,
) -> bool {
    let locktime_grin = slate.pub_slate.mw.lock_time.unwrap();
    let locktime_btc = slate.pub_slate.btc.lock_time.unwrap();

    let block_height_grin = grin_core.get_block_height().unwrap();
    let block_height_btc = btc_core.get_current_block_height().unwrap();

    return ((block_height_grin + (60 / GRIN_BLOCK_TIME)) <= u64::try_from(locktime_grin).unwrap())
        && ((block_height_btc + (60 / BTC_BLOCK_TIME)) <= u64::try_from(locktime_btc).unwrap());
}

/// If block times have passed the respective lock times we can refund
///
/// # Arguments
///
/// * `grin_core` Grin core node functions
/// * `btc_core` Bitcoin core node functions
/// * `slate` Swap slate
fn can_refund(grin_core: &mut GrinCore, btc_core: &mut BitcoinCore, slate: &SwapSlate) -> bool {
    let locktime_grin = slate.pub_slate.mw.lock_time.unwrap();
    let locktime_btc = slate.pub_slate.btc.lock_time.unwrap();

    let block_height_grin = grin_core.get_block_height().unwrap();
    let block_height_btc = btc_core.get_current_block_height().unwrap();

    return (block_height_grin > u64::try_from(locktime_grin).unwrap())
        && (block_height_btc > u64::try_from(locktime_btc).unwrap());
}
