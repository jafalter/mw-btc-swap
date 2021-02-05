use crate::{constants::SIGHASH_ALL, util};
use bitcoin::{PubkeyHash, blockdata::opcodes::{self, all::{OP_ELSE, OP_ENDIF}}, consensus::encode::deserialize, consensus::encode::serialize_hex, secp256k1::Signature};
use bitcoin::blockdata::opcodes::all::OP_CSV;
use bitcoin::blockdata::opcodes::all::OP_CLTV;
use bitcoin::blockdata::opcodes::all::OP_DROP;
use bitcoin::blockdata::opcodes::all::OP_IF;
use bitcoin::blockdata::script::Builder;
use bitcoin::Transaction;
use bitcoin::TxOut;
use opcodes::{OP_FALSE, OP_TRUE, all::{OP_CHECKMULTISIG, OP_CHECKMULTISIGVERIFY, OP_CHECKSIG, OP_CHECKSIGVERIFY}};
use serde_json::to_vec;
use crate::constants::FFFFFFFF;
use bitcoin::Script;
use bitcoin::OutPoint;
use bitcoin::Txid;
use bitcoin::TxIn;
use crate::bitcoin::bitcoin_types::BTCInput;
use bitcoin::PublicKey;
use rand::rngs::OsRng;
use bitcoin::PrivateKey;
use bitcoin::secp256k1::key::SecretKey;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::Message;
use bitcoin::secp256k1::All;
use bitcoin::util::address::Address;
use crate::constants::TEST_NET;
use bitcoin::network::constants::Network;
use bitcoin::hashes::sha256d::Hash;
use std::str::FromStr;

/// Creates a new secp256k1 private key used in bitcoin
/// 
/// # Arguments
/// 
/// * `rng` Randomness generator
pub fn create_private_key(rng : &mut OsRng) -> PrivateKey {
    let skey = SecretKey::new(rng);
    let nw = if TEST_NET { Network::Testnet } else { Network::Bitcoin };
    PrivateKey {
        compressed : true,
        network : nw,
        key : skey
    }
}

/// Build the bitcoin locking transaction which creates a P2SH transaction only spendable by recv_pk if he or she
/// gets access to the discrete log x of X = g^x or by refund_pk after refund_time
/// 
/// # Arguments
/// 
/// * `recv_pk` the receivers public key
/// * `pub_x` the statement pub_x = g^x for which the receivers needs to get x in order to spend this ouput
/// * `refund_pk` the public key of the sender which can be spent after refund time refund_time
/// * `change_pk` the change coin output public key
/// * `inputs` the inputs spend in this transaction
/// * `amount` the amount which should be locked
/// * `fee` the miners fee
/// * `refund_time` timelock for when this output should be spendable be the refunder
pub fn create_lock_transaction(recv_pk : PublicKey, pub_x : PublicKey, refund_pk : PublicKey, change_pk : PublicKey, inputs : Vec<BTCInput>, amount: u64, fee: u64, refund_time : i64) -> Result<Transaction,String> {
    let mut txinp : Vec<TxIn> = Vec::new();
    let mut txout : Vec<TxOut> = Vec::new();
    let mut inp_amount : u64 = 0;

    // Create the transaction inputs
    for btcinp in inputs {
        let txid = Txid::from_hash(Hash::from_str(&btcinp.txid)
            .expect("Failed to parse tx id from string"));
        let outpoint = OutPoint::new(txid, btcinp.vout);
        let script_sig = Script::new();
        let witness_data : Vec<Vec<u8>> = Vec::new();
        inp_amount = inp_amount + btcinp.value;
        txinp.push(TxIn{
            previous_output : outpoint,
            script_sig : script_sig,
            sequence : FFFFFFFF,
            witness : witness_data
        });
    }

    if inp_amount < (amount + fee) {
        Err(String::from("Input coin amount is too little"))
    }
    else {
        let lock_script_pub = get_lock_pub_script(recv_pk, pub_x, refund_pk, refund_time,true);
        txout.push(TxOut{
            value : amount,
            script_pubkey : lock_script_pub
        });

        // Change output
        let ch_script_pub = get_p2pkh_pub_script(&change_pk);
        txout.push(TxOut{
            value : inp_amount - amount - fee,
            script_pubkey : ch_script_pub
        });

        Ok(Transaction {
            version : 1,
            lock_time : 0,
            input: txinp,
            output: txout
        })
    }
}

/// Create a unsigned transaction fully spending the locked input to a p2pkh output
///
/// # Arguments
///
/// * `recv_pk` key to be used for the p2pkh output
/// * `input` the locked input to be spend
/// * `amount` the amount for the output
/// * `fee` the fee given to miners, note that amount + fee must equal the value of the locked coin
pub fn create_spend_lock_transaction(recv_pk : &PublicKey, input : BTCInput, amount : u64, fee : u64, lock_time : u32) -> Result<Transaction,String> {
    let mut txinp : Vec<TxIn> = Vec::new();
    let mut txout : Vec<TxOut> = Vec::new();

    let inp_amount : u64 = input.value;

    // Add the input
    let txid = Txid::from_hash(Hash::from_str(&input.txid).unwrap());
    let outpoint = OutPoint::new(txid, input.vout);
    let script_sig = Script::new();
    let witness_data : Vec<Vec<u8>> = Vec::new();
    txinp.push(TxIn {
        previous_output : outpoint,
        script_sig : script_sig,
        sequence : if lock_time == 0 { FFFFFFFF } else { 0 },
        witness : witness_data
    });

    if inp_amount != amount {
        Err(String::from("Please fully redeem input coin"))
    }
    else {
        let out_script = Script::new_p2pkh(&recv_pk.pubkey_hash());
        txout.push(TxOut{
            value : amount - fee,
            script_pubkey : out_script
        });

        Ok(Transaction {
            version : 1,
            lock_time : lock_time,
            input : txinp,
            output : txout
        })
    }
}

/// Create the transaction output which is a single P2SH script of the form
/// OP_IF 
///  <refund_time>
///  OP_CHECKLOCKTIMEVERIFY
///  OP_DROP
///  <refund_pub_key>
///  OP_CHECKSIGVERIFY
/// OP_ELSE
///  2 <recv_pub_key> <X> 2 CHECKMULTISIGVERIFY
/// OP_ENDIF
/// # Arguments
/// 
/// * `recv_pk` the receivers public key
/// * `pub_x` the statement pub_x = g^x for which the receivers needs to get x in order to spend this ouput
/// * `refund_pk` the public key of the sender which can be spent after refund time refund_time
/// * `refund_time` timelock for when this output should be spendable be the refunder
pub fn get_lock_pub_script(recv_pk : PublicKey, pub_x : PublicKey, refund_pk : PublicKey, refund_time : i64, wrap_in_p2sh : bool) -> Script {
    let builder = Builder::new()
        .push_opcode(OP_IF)
        .push_int(refund_time)
        .push_opcode(OP_CLTV)
        .push_opcode(OP_DROP)
        .push_key(&refund_pk)
        .push_opcode(OP_CHECKSIG)
        .push_opcode(OP_ELSE)
        .push_opcode(opcodes::all::OP_PUSHNUM_2)
        .push_key(&recv_pk)
        .push_key(&pub_x)
        .push_opcode(opcodes::all::OP_PUSHNUM_2)
        .push_opcode(OP_CHECKMULTISIG)
        .push_opcode(OP_ENDIF);
    if wrap_in_p2sh {
        Script::new_p2sh(&builder.into_script().script_hash())
    }
    else {
        builder.into_script()
    }
    
}

/// Create a P2PKH transaction output
///
/// # Arguments
///
/// * `recv_pk` the receiver public key
pub fn get_p2pkh_pub_script(recv_pk : &PublicKey) -> Script {
    Script::new_p2pkh(&recv_pk.pubkey_hash())
}

/// Converts a bitcoin pub script into a Bitcoin Address
/// 
/// # Arguments
/// 
/// * `s` the bitcoin script to convert into a address
pub fn script_to_address(s : Script) -> Address {
    let nw = if TEST_NET { Network::Testnet } else { Network::Bitcoin };
    Address::from_script(&s, nw).unwrap()
}

/// Sign a bitcoin transaction spending P2PKH outputs
/// 
/// # Arguments
/// 
/// * `tx` the transaction to be signed
/// * `script_pubkeys` list of script pubkeys of the inputs (has to be indexed in the same order as the inputs in the transaction)
/// * `skeys` list of secret keys with which to sign the inputs (has to be indexed in the same order as the inputs in the transaction)
/// * `pkeys` list of public keys for the inputs which are spent (has to be indexed in the same order as the inputs in the transaction)
/// * `curve` reference to the elliptic curve object used for signing
pub fn sign_p2pkh_transaction(tx : Transaction, script_pubkeys : Vec<Script>, skeys : Vec<PrivateKey>, pkeys: Vec<PublicKey>, secp : &Secp256k1<All>) -> Transaction {
    let mut signed_inp : Vec<TxIn> = Vec::new();

    for (i, unsigned_inp) in tx.input.iter().enumerate() {
        let script_pubkey = script_pubkeys.get(i).unwrap();
        let signing_key = skeys.get(i).unwrap().key;
        let pub_key = pkeys.get(i).unwrap();
        let sighash = tx.signature_hash(i, &script_pubkey, SIGHASH_ALL.into());
        let msg = Message::from_slice(&sighash.as_ref()).unwrap();
        let sig = secp.sign(&msg, &signing_key);
        let sig_der = serialize_sig_der_with_sighash(&sig, SIGHASH_ALL);

        // Standard P2PKH redeem script:
        // <sig> <pubkey>
        let redeem_script = Builder::new()
            .push_slice(&sig_der)
            .push_key(&pub_key)
            .into_script();
        signed_inp.push(TxIn {
            previous_output : unsigned_inp.previous_output,
            script_sig: redeem_script,
            sequence : unsigned_inp.sequence,
            witness : unsigned_inp.witness.clone()
        });
    }
    
    Transaction {
        version : tx.version,
        lock_time : tx.lock_time,
        input : signed_inp,
        output: tx.output
    }
}

/// Serialize a signature into bytes with the SIGHASH flag appened inside
/// the signature which is how Bitcoin Script expects it
///
/// # Arguments
/// 
/// * `sig` the signature to serialize
/// * `sig_hash` the SIGHASH flag to be appended
pub fn serialize_sig_der_with_sighash(sig : &Signature, sig_hash : u8) -> Vec<u8> {
    let mut sig_der = sig.serialize_der().to_vec();
    sig_der.push(sig_hash);
    sig_der
}

/// Sign a transaction spending the lock P2SH output as the redeemer
/// Returns a version of the transaction in which the locked input
/// is signed 
///
/// # Arguments
///
/// * `tx` the transaction spending from the locked output
/// * `ix` the index of the input to spend
/// * `lock_script` The locking script we need to present 
/// * `sk` the private key with which to sign
/// * `x` the secrete witness with which to sign
/// * `secp` curve functionalities
pub fn sign_lock_transaction_redeemer(tx : Transaction, ix : usize, lock_script : Script, sk : PrivateKey, x : PrivateKey, secp : &Secp256k1<All>) -> Transaction {
    let mut signed_inp : Vec<TxIn> = Vec::new();

    // Iterate through intputs and sign for input at index ix
    for (i, unsigned_inp) in tx.input.iter().enumerate() {
        if i == ix {
            let sighash = tx.signature_hash(ix, &lock_script, SIGHASH_ALL.into());
            let msg = Message::from_slice(&sighash.as_ref())
                .unwrap();
            let sig_a = secp.sign(&msg, &sk.key);
            let sig_x = secp.sign(&msg, &x.key);
            let sig_a_der = serialize_sig_der_with_sighash(&sig_a, SIGHASH_ALL);
            let sig_x_der = serialize_sig_der_with_sighash(&sig_x, SIGHASH_ALL);
            let lock_script_bytes: Vec<u8> = lock_script.to_bytes();
            // Now we need to combine the original P2SH script and the actual redeem script
            let fin_script = Builder::new()
                .push_opcode(opcodes::all::OP_PUSHBYTES_0)
                .push_slice(&sig_a_der)
                .push_slice(&sig_x_der)
                .push_opcode(opcodes::all::OP_PUSHBYTES_0) // To make the IF validate to false
                .push_slice(&lock_script_bytes)
                .into_script();
            signed_inp.push(TxIn {
                previous_output : unsigned_inp.previous_output,
                script_sig : fin_script,
                sequence : unsigned_inp.sequence,
                witness : unsigned_inp.witness.clone()
            });
        }
        else {
            signed_inp.push(unsigned_inp.clone());
        }
    }

    Transaction {
        version : tx.version,
        lock_time : tx.lock_time,
        input : signed_inp,
        output : tx.output
    }
}

/// Sign a transaction spending the lock P2SH output as the refunder
/// Returns a version of the transaction in which the locked input
/// is signed.
/// The transaction will only validate once the locktime was reached
pub fn sign_lock_transaction_refund(tx : Transaction, ix : usize, lock_script : Script, sk: PrivateKey, secp : &Secp256k1<All>) -> Transaction {
    let mut signed_inp : Vec<TxIn> = Vec::new();

    // Iterate through inputs and sign for input at index ix
    for (i, unsigned_inp) in tx.input.iter().enumerate() {
        if i == ix {
            let sighash = tx.signature_hash(ix, &lock_script, SIGHASH_ALL.into());
            let msg = Message::from_slice(&sighash.as_ref())
                .unwrap();
            let sig = secp.sign(&msg, &sk.key);
            let sig_der = serialize_sig_der_with_sighash(&sig, SIGHASH_ALL);
            let lock_script_bytes : Vec<u8> = lock_script.to_bytes();
            let fin_script = Builder::new()
                .push_slice(&sig_der)
                .push_int(1) // To make the OP_IF evaluate to true
                .push_slice(&lock_script_bytes)
                .into_script();
            signed_inp.push(TxIn {
                previous_output : unsigned_inp.previous_output,
                script_sig : fin_script,
                sequence : unsigned_inp.sequence, // Need to be 0 for the OP_CHECKLOCKTIME to verify
                witness : unsigned_inp.witness.clone()
            })
        }
        else {
            signed_inp.push(unsigned_inp.clone());
        }
    }

    Transaction {
        version : tx.version,
        lock_time : tx.lock_time, // Needs to be set such that the OP_CHECKLOCKTIME verifies
        input : signed_inp,
        output : tx.output
    }
}

/// Create a Script object from a hex encoded string
/// Returns a Script object
///
/// # Arguments
///
/// * `script_str` a hexencoded string representing a bitcoin script
pub fn deserialize_script(script_str : &String) -> Script {
    let bytes = hex::decode(script_str)
        .unwrap();
    Script::from(bytes)
}

/// Serializes a Bitcoin Script object into a string
/// Returns a hex encoded string
///
/// # Arguments
///
/// * `script` the bitcoin script object
pub fn serialize_script(script : &Script) -> String {
    hex::encode(script.to_bytes())
}

/// Serialize a bitcoin transaction into a string
/// Returns a hex encoded transaction string
///
/// # Arguments
///
/// * `tx` the transaction object
pub fn serialize_btc_tx(tx : &Transaction) -> String {
    serialize_hex(tx)
}

/// Deserialize a bitcoin transaction from a string
/// Returns a bitcoin transaction object
///
/// # Arguments
///
/// * `str_tx` the transaction as a hex encoded string
pub fn deserialize_btc_tx(str_tx : &String) -> Transaction {
    deserialize(&hex::decode(str_tx).unwrap())
        .unwrap()
}

/// Serialize a Bitcoin PrivateKey to a string
/// Returns a bitcoin private key in wif format
///
/// # Arguments
///
/// * `sk` the secret key to be serialized
pub fn serialize_priv_key(sk : &PrivateKey) -> String {
    sk.to_wif()
}

/// Deserializes a Bitcoin PrivateKey from a string
/// Returns a deserialized PrivateKey object
///
/// # Arguments
///
/// * `sk` Secretkey in wif format
pub fn deserialize_priv_key(sk: &String) -> PrivateKey {
    PrivateKey::from_wif(&sk).unwrap()
}

/// Serialize a Bitcoin PublicKey to a string
/// Returns a bitcoin public key serialized to a string
///
/// # Arguments
///
/// * `pk` the public key to be serialized
pub fn serialize_pub_key(pk : &PublicKey) -> String {
    pk.to_string()
}

/// Deserialize a Bitcoin PublicKey from a string
/// Returns a Bitcoin public key object
///
/// # Arguments
///
/// * `str` serialized Bitcoin public key
pub fn deserialize_pub_key(str : &String) -> PublicKey {
    PublicKey::from_str(str).unwrap()
}

#[test]
fn test_script_serialization() {
    let hex_script = String::from("0014ebcf32c56219bb6782aa51895451f1d818b50af5");
    let script = deserialize_script(&hex_script);
    let serialized = serialize_script(&script);
    assert_eq!(hex_script, serialized);
}

#[test]
fn test_tx_serialization() {
    let tx_str = String::from("0100000001a5b9ee765b9d78bb40e7c24005246de8aedf796089474a187041b45c3183ebe3000000006a473045022100e328a3960f10a5d24fda55fedcc71a88d5b0ff431029cd7568f2f0076bcf2a8b022018e57b708b2ad18916296b1cef625468c889064d65bca304e5a8a9e5a4f692172103c7eafa9bb32d43b88580ddd259aab1c76b8f1749ae43a343add030884edaae99ffffffff01be0000000000000017a91424f9fd677d9f32cdf976cf0ca146d55a3ece4d038700000000");
    let tx = deserialize_btc_tx(&tx_str);
    let serialized = serialize_btc_tx(&tx);
    assert_eq!(tx_str, serialized);
}

#[test]
fn test_create_lock_tx() {
    let inp_key_wif = String::from("cTMx4ZMVW8b1JrfURdi8pqRN8a2yKn66WQLhLho9gpPtFsuTXMPg");
    let inp_value = 1927496;
    let inp_vout = 0;
    let inp_txid = String::from("dee1ed3f6c305e13004ca34ccc0dfc0bb7af10f647c4c880ad38a4a4b63ee5f5");
    let inp_pub_script = String::from("76a914af379fcf3f457c464f50be456a49a6f22019c73088ac");
    let refund_time = 1906254;

    let mut rng = util::get_os_rng();
    let secp = util::get_secp256k1_curve(); 
    let inp_key = PrivateKey::from_wif(&inp_key_wif)
        .unwrap();
    let inp_pk = PublicKey::from_private_key(&secp, &inp_key);
    let inp_pk_hex = hex::encode(&inp_pk.to_bytes()); 

    let bob_sk = create_private_key(&mut rng);
    let alice_sk = create_private_key(&mut rng);
    let x = create_private_key(&mut rng);
    let change_sk = create_private_key(&mut rng);
    let bob_pk = PublicKey::from_private_key(&secp, &bob_sk);
    let alice_pk = PublicKey::from_private_key(&secp, &alice_sk);
    let pub_x = PublicKey::from_private_key(&secp, &x);
    let pub_ch = PublicKey::from_private_key(&secp, &change_sk);
    println!("Bob sk: {}", bob_sk.to_wif());
    println!("Alice sk: {}", alice_sk.to_wif());
    println!("x: {}", x.to_wif());
    println!("Change output sk: {}", change_sk.to_wif());
    let inp = BTCInput::new(
        inp_txid, 
        inp_vout, 
        inp_value, 
        inp_key_wif,
        String::from(inp_pk_hex), 
        inp_pub_script
    );
    let tx = create_lock_transaction(alice_pk, pub_x, bob_pk, pub_ch, vec![inp.clone()], 100000, 500, refund_time)
        .unwrap();
    let inp_script = deserialize_script(&inp.pub_script);
    let signed_tx = sign_p2pkh_transaction(tx, vec![inp_script], vec![inp_key], vec![inp_pk], &secp);
    let str_tx = serialize_btc_tx(&signed_tx);
    println!("Change output sk: {}", change_sk.to_wif());

    println!("{}", str_tx);
}

#[test]
fn test_redeem_from_lock_tx() {
    let txid = String::from("3f11e68ec0798b3f550c99b232353f51ba9a2442c731580e521777c79c1829da");
    let bob_sk_wif = String::from("cVzf65djFRbgYN6W4iwSn6Use2S7jQtGhsYYQTJxHnAPRowbNF5N");
    let alice_sk_wif = String::from("cPpqNPQtBMh8GRcxD8e34UdergNbNFVFbETcRyt1wn57r8VsS8VV");
    let x_wif = String::from("cT3iDo6QMVkNhwXLjdWBgVJhPjUMG1At7uoBRKeuHT2qoSY7wteq");
    let inp_pub_script = String::from("a914c705426ecd4b427caefd4530d6f78c732b40956b87");
    let vout = 0;
    let refund_time = 1906254;
    let inp_value = 100000;
    let fee = 500;
    
    let mut rng = util::get_os_rng();
    let secp = util::get_secp256k1_curve();

    let bob_sk = PrivateKey::from_wif(&bob_sk_wif)
        .unwrap();
    let alice_sk = PrivateKey::from_wif(&alice_sk_wif)
        .unwrap();
    let x = PrivateKey::from_wif(&x_wif)
        .unwrap();

    let recv_key = create_private_key(&mut rng);
    let pub_recv = PublicKey::from_private_key(&secp, &recv_key);
    println!("Receivers secret key: {}", recv_key.to_wif());

    let inp = BTCInput::new(txid, vout, inp_value, alice_sk.to_wif(), PublicKey::from_private_key(&secp, &alice_sk).to_string(), inp_pub_script.clone());

    let tx = create_spend_lock_transaction(&pub_recv, inp, inp_value, fee, 0)
        .unwrap();
    let lock_script = get_lock_pub_script(
        PublicKey::from_private_key(&secp, &alice_sk), 
        PublicKey::from_private_key(&secp, &x), 
        PublicKey::from_private_key(&secp, &bob_sk),
         refund_time, false);

    let signed_tx = sign_lock_transaction_redeemer(tx, 0, lock_script, alice_sk, x, &secp);
    let ser_tx = serialize_btc_tx(&signed_tx);
    println!("{}", ser_tx);
}

#[test]
fn test_refund_from_lock_tx() {
    let txid = String::from("3f11e68ec0798b3f550c99b232353f51ba9a2442c731580e521777c79c1829da");
    let bob_sk_wif = String::from("cVzf65djFRbgYN6W4iwSn6Use2S7jQtGhsYYQTJxHnAPRowbNF5N");
    let alice_sk_wif = String::from("cPpqNPQtBMh8GRcxD8e34UdergNbNFVFbETcRyt1wn57r8VsS8VV");
    let x_wif = String::from("cT3iDo6QMVkNhwXLjdWBgVJhPjUMG1At7uoBRKeuHT2qoSY7wteq");
    let inp_pub_script = String::from("a914c705426ecd4b427caefd4530d6f78c732b40956b87");
    let vout = 0;
    let refund_time : u32 = 1906254;
    let inp_value = 100000;
    let fee = 500;
    let mut rng = util::get_os_rng();
    let secp = util::get_secp256k1_curve();

    let bob_sk = PrivateKey::from_wif(&bob_sk_wif)
        .unwrap();
    let alice_sk = PrivateKey::from_wif(&alice_sk_wif)
        .unwrap();
    let x = PrivateKey::from_wif(&x_wif)
        .unwrap();

    let recv_key = create_private_key(&mut rng);
    let pub_recv = PublicKey::from_private_key(&secp, &recv_key);
    println!("Receivers secret key: {}", recv_key.to_wif());

    let inp = BTCInput::new(txid, vout, inp_value, alice_sk.to_wif(), PublicKey::from_private_key(&secp, &alice_sk).to_string(), inp_pub_script.clone());

    let tx = create_spend_lock_transaction(&pub_recv, inp, inp_value, fee, refund_time)
        .unwrap();
    let lock_script = get_lock_pub_script(
        PublicKey::from_private_key(&secp, &alice_sk), 
        PublicKey::from_private_key(&secp, &x), 
        PublicKey::from_private_key(&secp, &bob_sk),
         refund_time.into(), false);
    let signed_tx = sign_lock_transaction_refund(tx, 0, lock_script, bob_sk, &secp);
    let ser_tx = serialize_btc_tx(&signed_tx);
    println!("{}", ser_tx);
}
