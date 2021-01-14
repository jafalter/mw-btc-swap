use std::net::TcpStream;

use crate::net::tcp::{read_from_stream, write_to_stream};
use grin_wallet_libwallet::Slate;

use super::{grin_core::GrinCore, grin_types::MWCoin};

pub struct GrinTx {
    core: GrinCore,
}

pub struct DBuildMWTxResult {
    tx : Slate,
    coin : MWCoin
}

pub struct DSharedOutMwTxResult {

}

impl GrinTx {
    pub fn new() -> GrinTx {
        let core = GrinCore::new();
        GrinTx { core: core }
    }

    /// The Alice part of the dBuildMWTx protocol of the thesis
    /// Communicate via TCP Stream with Bob to build a mimblewimble transaction
    ///
    /// # Arguments
    ///
    /// * `inp` the input coins to spend
    /// * `fund_value` the amount of nanogrin to send with this transaction
    /// * `timelock` optional transaction timelock
    /// * `stream` the stream on which we communicate with the second party
    pub fn dbuildmw_tx_alice(
        &mut self,
        inp: Vec<MWCoin>,
        fund_value: u64,
        timelock: u64,
        stream: &mut TcpStream,
    ) -> Result<DBuildMWTxResult, String> {
        // Create initial pre-transaction by calling spend coins
        let spend_coins_result = self.core.spend_coins(inp, fund_value, timelock, 2, 2)?;
        // Send the pre-tx to Bob
        let ptx = serde_json::to_string(&spend_coins_result.slate).unwrap();
        write_to_stream(stream, &ptx);
        let bob_msg = read_from_stream(stream);
        let ptx2: Slate = Slate::deserialize_upgrade(&bob_msg)
            .unwrap();
        let fin = self.core.fin_tx(
            ptx2,
            &spend_coins_result.sig_key,
            &spend_coins_result.sig_nonce,
            true,
            None,
            None,
        ).unwrap();
        let tx = serde_json::to_string(&fin)
            .unwrap();
        write_to_stream(stream, &tx);
        Ok(DBuildMWTxResult {
            tx : fin,
            coin : spend_coins_result.change_coin.unwrap()
        })
    }

    /// The Bob part of the dBuildMwTx protocol from the thesis
    /// Communicate via TCP with Alice to build a Mimblewimble transaction
    ///
    /// # Arguments
    ///
    /// * `fund_value` the amount in nanogrin transfered with the transaction
    /// * `stream` the tcp stream on which we exchange messages with Alice
    pub fn dbuild_mw_tx_bob(
        &mut self,
        fund_value: u64,
        stream: &mut TcpStream
    ) -> Result<DBuildMWTxResult, String> {
        // Retrieve initial pre-transaction from Alice
        let mut alice_msg = read_from_stream(stream);
        let ptx : Slate = Slate::deserialize_upgrade(&alice_msg)
            .unwrap();
        // Now we create the updated pre-transaction
        let ptx2 = self.core.recv_coins(ptx, fund_value)
            .unwrap();
        // Send the updated pre-transaction back to Alice
        let ptx2_str = serde_json::to_string(&ptx2.slate)
            .unwrap();
        write_to_stream(stream, &ptx2_str);
        // Retrieve the final tx from Alice
        alice_msg = read_from_stream(stream);
        let tx = Slate::deserialize_upgrade(&alice_msg)
            .unwrap();
        Ok(DBuildMWTxResult {
            tx : tx,
            coin : ptx2.output_coin
        })
    }

    pub fn dshared_out_mw_tx_alice(
        &mut self,
        inp: Vec<MWCoin>,
        fund_value: u64,
        timelock: u64,
        stream: &mut TcpStream,
    ) -> Result<DSharedOutMwTxResult, String> {
        // Create the initial pre-transaction
        let spend_coins_result = self.core.spend_coins(inp, fund_value, timelock, 2, 3)
            .unwrap();
        // Run the first roud of the drecvcoins protocol
        let recv_coins_result = self.core.drecv_coins_r1(spend_coins_result.slate, fund_value)
            .unwrap();
        let ptx = serde_json::to_string(&recv_coins_result.slate)
            .unwrap();
        write_to_stream(stream, &ptx);
        let bob_msg = read_from_stream(stream);
        let ptx2 : Slate = Slate::deserialize_upgrade(&bob_msg)
            .unwrap();
        // round 3 of the recv coins

        Ok(DSharedOutMwTxResult {

        })
    }

    pub fn dshared_out_mw_tx_bob(

    ) -> Result<DSharedOutMwTxResult, String> {
        Ok(DSharedOutMwTxResult {
            
        })
    }
}
