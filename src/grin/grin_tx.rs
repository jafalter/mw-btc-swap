use std::net::TcpStream;

use crate::net::tcp::{read_from_stream, write_to_stream};
use grin_wallet_libwallet::Slate;

use super::{grin_core::GrinCore, grin_types::MWCoin};

pub struct GrinTx {
    core: GrinCore,
}

pub struct DBuildMWTxAlice {
    tx : Slate,
    coin : MWCoin
}

impl GrinTx {
    pub fn new() -> GrinTx {
        let core = GrinCore::new();
        GrinTx { core: core }
    }

    pub fn dbuild_mw_tx_alice(
        &mut self,
        inp: Vec<MWCoin>,
        fund_value: u64,
        timelock: u64,
        stream: &mut TcpStream,
    ) -> Result<DBuildMWTxAlice, String> {
        // Create initial pre-transaction by calling spend coins
        let spend_coins_result = self.core.spend_coins(inp, fund_value, timelock, 2, 2)?;
        // Send the pre-tx to Bob
        let ptx = serde_json::to_string(&spend_coins_result.slate).unwrap();
        write_to_stream(stream, &ptx);
        let bobs_response = read_from_stream(stream);
        let ptx2: Slate = Slate::deserialize_upgrade(&bobs_response)
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
        Ok(DBuildMWTxAlice {
            tx : fin,
            coin : spend_coins_result.change_coin.unwrap()
        })
    }
}
