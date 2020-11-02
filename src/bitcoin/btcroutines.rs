use crate::constants::SIGHASH_ALL;
use bitcoin::blockdata::opcodes::all::OP_ELSE;
use bitcoin::blockdata::opcodes::all::OP_CSV;
use bitcoin::blockdata::opcodes::all::OP_CLTV;
use bitcoin::blockdata::opcodes::all::OP_DROP;
use bitcoin::blockdata::opcodes::all::OP_IF;
use bitcoin::blockdata::script::Builder;
use bitcoin::Transaction;
use bitcoin::TxOut;
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
use crate::constants::TEST_NET;
use bitcoin::network::constants::Network;
use bitcoin::hashes::sha256d::Hash;
use std::str::FromStr;

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
/// * `X` the statement X = g^x for which the receivers needs to get x in order to spend this ouput
/// * `refund_pk` the public key of the sender which can be spent after refund time refund_time
/// * `inputs` the inputs spend in this transaction
/// * `amount` the amount which should be locked
/// * `fee` the miners fee
pub fn create_lock_transaction(recv_pk : PublicKey, X : PublicKey, refund_pk : PublicKey, inputs : Vec<BTCInput>, amount: u64, fee: u64, refund_time : i64) -> Transaction {
    let mut txinp : Vec<TxIn> = Vec::new();
    let mut txout : Vec<TxOut> = Vec::new();

    // Create the transaction inputs
    for btcinp in inputs {
        let txid = Txid::from_hash(Hash::from_str(&btcinp.txid)
            .expect("Failed to parse tx id from string"));
        let outpoint = OutPoint::new(txid, btcinp.vout);
        let script_sig = Script::new();
        let witness_data : Vec<Vec<u8>> = Vec::new();
        txinp.push(TxIn{
            previous_output : outpoint,
            script_sig : script_sig,
            sequence : FFFFFFFF,
            witness : witness_data
        });
    }

    // Create the transaction output which is a single P2SH script of the form
    // OP_IF 
    //  <refund_time>
    //  OP_CHECKLOCKTIMEVERIFY
    //  OP_DROP
    //  <refund_pub_key>
    //  OP_CHECKSIGVERIFY
    // OP_ELSE
    //  2 <recv_pub_key> <X> 2 CHECKMULTISIGVERIFY
    let builder = Builder::new()
        .push_opcode(OP_IF)
        .push_int(refund_time)
        .push_opcode(OP_CLTV)
        .push_opcode(OP_DROP)
        .push_key(&refund_pk)
        .push_opcode(OP_CSV)
        .push_opcode(OP_ELSE)
        .push_int(2)
        .push_key(&recv_pk)
        .push_key(&X)
        .push_int(2)
        .push_opcode(OP_CSV);

    let script_pub = Script::new_p2sh(&builder.into_script().script_hash());
    txout.push(TxOut{
        value : amount - fee,
        script_pubkey : script_pub
    });

    Transaction {
        version : 1,
        lock_time : 0,
        input: txinp,
        output: txout
    }
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
pub fn sign_transaction(tx : Transaction, script_pubkeys : Vec<Script>, skeys : Vec<PrivateKey>, pkeys: Vec<PublicKey>, curve : &Secp256k1<All>) -> Transaction {
    let mut signed_inp : Vec<TxIn> = Vec::new();

    for (i, unsigned_inp) in tx.input.iter().enumerate() {
        let script_pubkey = script_pubkeys.get(i).unwrap();
        let signing_key = skeys.get(i).unwrap().key;
        let pub_key = pkeys.get(i).unwrap();
        let sighash = tx.signature_hash(i, &script_pubkey, SIGHASH_ALL);
        let msg = Message::from_slice(&sighash.as_ref()).unwrap();
        let sig = curve.sign(&msg, &signing_key);
        // Standard P2PKH redeem script:
        // <sig> <pubkey>
        let redeem_script = Builder::new()
            .push_slice(sig.serialize_der().as_ref())
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
