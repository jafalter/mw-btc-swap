use std::net::TcpStream;

use crate::{net::{http::RequestFactory, tcp::{receive_msg, send_msg}}, settings::GrinNodeSettings};
use grin_util::secp::{PublicKey, SecretKey};
use grin_wallet_libwallet::Slate;

use super::{grin_core::GrinCore, grin_routines::{MPBPContext, sig_extract_s}, grin_types::MWCoin};

pub struct GrinTx {
    core: GrinCore,
}

pub struct DBuildMWTxResult {
    pub tx: Slate,
    pub coin: Option<MWCoin>,
}

pub struct DSharedOutMwTxResult {
    pub tx: Slate,
    pub change_coin: Option<MWCoin>,
    pub shared_coin: MWCoin,
}

pub struct ContractMwResult {
    pub tx : Slate,
    pub coin : Option<MWCoin>,
    pub x : SecretKey
}

impl GrinTx {
    pub fn new(settings : GrinNodeSettings, req_factory : RequestFactory) -> GrinTx {
        let core = GrinCore::new(settings, req_factory);
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
        send_msg(stream, &ptx);
        let bob_msg = receive_msg(stream);
        let ptx2: Slate = Slate::deserialize_upgrade(&bob_msg).unwrap();
        let fin = self
            .core
            .fin_tx(
                ptx2,
                &spend_coins_result.sig_key,
                &spend_coins_result.sig_nonce,
                true,
                None,
                None,
            )
            .unwrap();
        let tx = serde_json::to_string(&fin).unwrap();
        send_msg(stream, &tx);
        Ok(DBuildMWTxResult {
            tx: fin,
            coin: spend_coins_result.change_coin,
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
        stream: &mut TcpStream,
    ) -> Result<DBuildMWTxResult, String> {
        // Retrieve initial pre-transaction from Alice
        let mut alice_msg = receive_msg(stream);
        let ptx: Slate = Slate::deserialize_upgrade(&alice_msg).unwrap();
        // Now we create the updated pre-transaction
        let ptx2 = self.core.recv_coins(ptx, fund_value).unwrap();
        // Send the updated pre-transaction back to Alice
        let ptx2_str = serde_json::to_string(&ptx2.slate).unwrap();
        send_msg(stream, &ptx2_str);
        // Retrieve the final tx from Alice
        alice_msg = receive_msg(stream);
        let tx = Slate::deserialize_upgrade(&alice_msg).unwrap();
        Ok(DBuildMWTxResult {
            tx: tx,
            coin: Some(ptx2.output_coin),
        })
    }

    /// The Alice part of the dsharedOutMWTx protocol from the thesis
    /// Communicate via Stream with Bob to build a Mimblewimble transaction in which
    /// the output coin is shared between Alice and Bob
    ///
    /// # Arguments
    ///
    /// * `fund_value` the transaction amount for the shared output coin
    /// * `timelock` optional block height to timelock the transaction
    /// * `stream` TCP stream to exchange messages with bob
    pub fn dshared_out_mw_tx_alice(
        &mut self,
        inp: Vec<MWCoin>,
        fund_value: u64,
        timelock: u64,
        stream: &mut TcpStream,
    ) -> Result<DSharedOutMwTxResult, String> {
        // Create the initial pre-transaction
        let spend_coins_result = self
            .core
            .spend_coins(inp, fund_value, timelock, 2, 3)
            .unwrap();
        // Run the first round of the drecvcoins protocol
        let mut recv_coins_result = self
            .core
            .drecv_coins_r1(spend_coins_result.slate, fund_value)
            .unwrap();
        let ptx = serde_json::to_string(&recv_coins_result.slate).unwrap();
        send_msg(stream, &ptx);
        send_msg(stream, &recv_coins_result.prf_ctx.to_string());
        let bob_msg = receive_msg(stream);
        let bob_msg2 = receive_msg(stream);
        let ptx2: Slate = Slate::deserialize_upgrade(&bob_msg).unwrap();
        let prf_ctx = MPBPContext::from_string(&bob_msg2);
        // round 3 of the recv coins
        let drecv_coins_result3 = self
            .core
            .drecv_coins_r3(
                ptx2,
                prf_ctx,
                recv_coins_result.out_key_blind,
                recv_coins_result.prf_nonce,
                recv_coins_result.sig_nonce,
            )
            .unwrap();
        // finalize the transaction
        let fin_slate = self
            .core
            .fin_tx(
                drecv_coins_result3.slate,
                &spend_coins_result.sig_key,
                &spend_coins_result.sig_nonce,
                true,
                None,
                None,
            )
            .unwrap();
        // Send final tx to bob
        let tx = serde_json::to_string(&fin_slate).unwrap();
        send_msg(stream, &tx);

        Ok(DSharedOutMwTxResult {
            tx: fin_slate,
            change_coin: spend_coins_result.change_coin,
            shared_coin: drecv_coins_result3.output_coin,
        })
    }

    /// The Bob part of the dsharedOutMWTx protocol from the thesis.
    /// Communicate via a stream with Alice to build a transaction with an
    /// output coin shared between Alice and Bob
    ///
    /// # Arguments
    ///
    /// * `fund_value` the transaction amount for the shared output coin
    /// * `stream` channel to communicate with Alice
    pub fn dshared_out_mw_tx_bob(
        &mut self,
        fund_value: u64,
        stream: &mut TcpStream,
    ) -> Result<DSharedOutMwTxResult, String> {
        // Read the initial pre-transaction from Alice
        let alice_msg1 = receive_msg(stream);
        // Read the multi-party rangeproof context
        let alice_msg2 = receive_msg(stream);
        let ptx = Slate::deserialize_upgrade(&alice_msg1).unwrap();
        let prf_ctx = MPBPContext::from_string(&alice_msg2);
        let mut drecv_coins2_result = self.core.drecv_coins_r2(ptx, fund_value, prf_ctx)?;
        // Send the updated pre-transaction to Alice
        let ptx2 = serde_json::to_string(&drecv_coins2_result.0.slate).unwrap();
        let prf_ctx2 = drecv_coins2_result.1.to_string();
        send_msg(stream, &ptx2);
        // Send the updated proof context
        send_msg(stream, &prf_ctx2);

        let tx_str = receive_msg(stream);
        let tx = Slate::deserialize_upgrade(&tx_str).unwrap();

        Ok(DSharedOutMwTxResult {
            tx: tx,
            change_coin: None,
            shared_coin: drecv_coins2_result.0.output_coin,
        })
    }

    /// Implementation of the Alice part of the dsharedInpMWTX
    /// In this type of transaction Alice and Bob cooperate to spend
    /// a coin which ownership is shared between the two participants
    ///
    /// # Arguments
    ///
    /// * `inp` the shared input coin
    /// * `fund_value` the value which should be transferred to Bob
    /// * `timelock` optional timelock
    /// * `steam` channel to exchange messages with Bob 
    pub fn dshared_inp_mw_tx_alice(
        &mut self,
        inp: MWCoin,
        fund_value: u64,
        timelock: u64,
        stream: &mut TcpStream,
    ) -> Result<DBuildMWTxResult, String> {
        let dspend_coins_result = self
            .core
            .spend_coins(vec![inp], fund_value, timelock, 1, 3)?;
        
        // Send initial slate to Bob
        let ptx = serde_json::to_string(&dspend_coins_result.slate)
            .unwrap();
        send_msg(stream, &ptx);
        // Receive updated pre-transaction from Bob
        let bob_msg = receive_msg(stream);
        let mut ptx2 = Slate::deserialize_upgrade(&bob_msg).unwrap();
        ptx2.update_kernel().unwrap();
        // Second round of finalize tx
        let fin_slate = self.core.fin_tx(
            ptx2,
            &dspend_coins_result.sig_key,
            &dspend_coins_result.sig_nonce,
            true,
            None,
            None,
        )?;
        let tx = serde_json::to_string(&fin_slate)
            .unwrap();
        // Send final tx to Bob
        send_msg(stream, &tx);
        Ok(DBuildMWTxResult {
            tx: fin_slate,
            coin: dspend_coins_result.change_coin,
        })
    }

    /// Implementation of the Bob part of the dSharedInpMWTx protocol
    /// Creates a transaction in which Alice and Bob cooperate to spend a
    /// coin which ownership is shared between them
    ///
    /// # Arguments
    ///
    /// * `inp` the shared input coin
    /// * `fund_value` the value which should be transferred to Bob
    /// * `timelock` optional transaction timelock
    /// * `stream` channel to communicate with Alice
    pub fn dshared_inp_mw_tx_bob(
        &mut self,
        inp: MWCoin,
        fund_value: u64,
        timelock: u64,
        stream: &mut TcpStream,
    ) -> Result<DBuildMWTxResult, String> {
        // Receive initial pre-transaction from alice
        let alice_msg = receive_msg(stream);
        let ptx = Slate::deserialize_upgrade(&alice_msg).unwrap();

        // Add our spending info
        let dspend_result = self
            .core
            .d_spend_coins(vec![inp], ptx, fund_value, timelock)?;
        // Create out output coin
        let recv_result = self.core.recv_coins(dspend_result.slate, fund_value)?;
        // First round of the dfin_tx
        let fin_result = self.core.fin_tx(
            recv_result.slate,
            &dspend_result.sig_key,
            &dspend_result.sig_nonce,
            false,
            None,
            None,
        ).unwrap();
        // Send the updated pre-tx to Alice
        let ptx2 = serde_json::to_string(&fin_result)
            .unwrap();
        send_msg(stream, &ptx2);

        // Read final tx from Alice
        let alice_msg = receive_msg(stream);
        let mut fin_slate = Slate::deserialize_upgrade(&alice_msg)
            .unwrap();
        fin_slate.update_kernel().unwrap();
        fin_slate.finalize(&self.core.chain).unwrap();
        Ok(DBuildMWTxResult{
            tx : fin_slate,
            coin : Some(recv_result.output_coin)
        })
    }

    /// The implementation of the Alice side of the dContractMWTX protocol from the thesis
    /// Creates a tx spending a shared input coin while revealing SecretKey x to Alice (for which she knows the PublicKey)
    /// It outputs the transaction, change coins and the secret witness
    ///
    /// # Arguments
    ///
    /// * `inp` shared input coin
    /// * `fund_value` the transaction value 
    /// * `timelock` optional to height lock a transaction
    /// * `pub_x` the public X for which Alice shall receive the x
    /// * `stream` channel to exchange messages with Bob
    pub fn dcontract_mw_tx_alice(
        &mut self,
        inp: MWCoin,
        fund_value : u64,
        timelock : u64,
        pub_x : PublicKey,
        stream : &mut TcpStream
    ) -> Result<ContractMwResult, String> {
        let dspend_coins_result = self
            .core
            .spend_coins(vec![inp], fund_value, timelock, 1, 3)?;
        let ptx = serde_json::to_string(&dspend_coins_result.slate)
            .unwrap();
        // Send initial slate to Bob
        send_msg(stream, &ptx);
        // Receive updated pre-transaction from Bob
        let bob_msg = receive_msg(stream);
        let ptx2 = Slate::deserialize_upgrade(&bob_msg)
            .unwrap();
        let apt_sig_bob = ptx2
            .participant_data
            .get(2)
            .unwrap()
            .part_sig
            .unwrap();
        // First round of finalize tx
        let fin_tx_result = self.core.fin_tx(
            ptx2, 
            &dspend_coins_result.sig_key, 
            &dspend_coins_result.sig_nonce, 
            false, 
            Some(pub_x), 
            None
        )?;
        let sig_alice = fin_tx_result
            .participant_data
            .get(0)
            .unwrap()
            .part_sig
            .unwrap();
        // Send ptx3 to Bob which he should then complete into the final tx
        let ptx3 = serde_json::to_string(&fin_tx_result)
            .unwrap();
        send_msg(stream, &ptx3);
        // Receive final tx from Bob
        let bob_msg2 = receive_msg(stream);
        let final_slate = Slate::deserialize_upgrade(&bob_msg2)
            .unwrap();
        let fin_sig = final_slate.tx.clone().unwrap().kernels()[0].excess_sig;

        // Finally we extract x from the signatures
        let s_fin = sig_extract_s(&fin_sig, &self.core.secp);
        let s_bob_apt = sig_extract_s(&apt_sig_bob, &self.core.secp);
        let s_alice = sig_extract_s(&sig_alice, &self.core.secp);
        let mut s_alice_neg = s_alice.clone();
        s_alice_neg.neg_assign(&self.core.secp).unwrap();
        let mut s_bob = s_fin.clone();
        s_bob.add_assign(
            &self.core.secp, 
            &s_alice_neg
        ).unwrap();
        let mut s_bob_neg = s_bob.clone();
        s_bob_neg.neg_assign(&self.core.secp).unwrap();
        let mut x = s_bob_apt.clone();
        x.add_assign(
            &self.core.secp,
            &s_bob_neg 
        ).unwrap();

        Ok(ContractMwResult{
            tx : final_slate,
            coin : dspend_coins_result.change_coin,
            x : x
        })
    }

    /// The implementation of the Bob side of the dContractMWTX transaction spending a shared input coin
    /// and revealing the secret key x = g^x to Alice in the process
    /// The function returns the transaction, output coin and the secret x
    ///
    /// # Arguments
    ///
    /// * `inp` the shared input coin
    /// * `fund_value` the transaction valu
    /// * `timelock` optional timelock
    /// * `x` the SecretKey for X = g^x
    /// * `stream` channel to exchange messages with Alice 
    pub fn dcontract_mw_tx_bob(
        &mut self,
        inp: MWCoin,
        fund_value : u64,
        timelock : u64,
        x : SecretKey,
        stream : &mut TcpStream
    ) -> Result<ContractMwResult, String> {
        // Receive initial pre-transaction from Alice
        let alice_msg = receive_msg(stream);
        let ptx = Slate::deserialize_upgrade(&alice_msg).unwrap();
        // Build updated pre-transaction
        let dspend_coins_result = self.core.d_spend_coins(vec![inp], ptx, fund_value, timelock)?;
        let rec_coins_result = self.core.apt_recv_coins(dspend_coins_result.slate, fund_value, x.clone())?;
        // Send to Alice the update pre-transaction
        let ptx2 = serde_json::to_string(&rec_coins_result.slate).unwrap();
        send_msg(stream, &ptx2);
        // Receive the partially finalized tx from alice
        let alice_msg2 = receive_msg(stream);
        let ptx3 = Slate::deserialize_upgrade(&alice_msg2).unwrap();
        // Finalize the transaction
        let fin_tx_result = self.core.fin_tx(
            ptx3, 
            &dspend_coins_result.sig_key, 
            &dspend_coins_result.sig_nonce, 
            true, 
            None,
            Some(rec_coins_result.prt_sig) 
        )?;
        // send final tx to alice
        let tx = serde_json::to_string(&fin_tx_result).unwrap();
        send_msg(stream, &tx);
        Ok(ContractMwResult{
            tx : fin_tx_result,
            coin : Some(rec_coins_result.output_coin),
            x : x
        })
    }

}
