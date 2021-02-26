use crate::{bitcoin::bitcoin_core::BitcoinCore, enums::SwapStatus, grin::{grin_core::GrinCore, grin_tx::GrinTx}, net::http::RequestFactory, swap::{protocol::exec_phase_swap_btc, protocol::exec_phase_swap_mw, slate::{self, get_slate_checksum, write_slate_to_disk}}};
use crate::swap::slate::read_slate_from_disk;
use rand::rngs::OsRng;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::All;
use crate::SwapSlate;
use crate::Settings;
use crate::commands::cmd_types::command::Command;
use std::net::{TcpStream};
use crate::net::tcp::send_msg;
use crate::net::tcp::receive_msg;
use crate::enums::SwapType;
use grin_util::secp::Secp256k1 as GrinSecp256k1;

/// Start Atomic Swap Execution
pub struct Execute {
    swapid : u64
}

impl Execute {
    pub fn new(swapid : u64) -> Execute {
        Execute {
            swapid : swapid
        }
    }
}

impl Command for Execute {
    fn execute(&self, settings : &Settings, rng : &mut OsRng, btc_secp : &Secp256k1<All>, grin_secp : &GrinSecp256k1) -> Result<SwapSlate, String> {
        let mut slate : SwapSlate = read_slate_from_disk(self.swapid, &settings.slate_directory)
            .expect("Unable to read slate files from disk");
        let mut stream : TcpStream = TcpStream::connect(format!("{}:{}", slate.pub_slate.meta.server, slate.pub_slate.meta.port))
            .expect("Failed to connect to peer via TCP");
        let mut btc_core = BitcoinCore::new(settings.btc.clone(), RequestFactory::new(None));
        let mut grin_core = GrinCore::new(settings.grin.clone(), RequestFactory::new(None));
        let mut grin_tx = GrinTx::new(settings.grin.clone(), RequestFactory::new(None));
        // first message exchanged is a hash of the pub slate file
        println!("Connected to peer");
        let checksum = get_slate_checksum(slate.id, &settings.slate_directory).unwrap();
        send_msg(&mut stream, &checksum);
        let resp = receive_msg(&mut stream);
        if resp.eq_ignore_ascii_case("OK") == false {
            Err(String::from("Checksums didn't match"))
        }
        else {
            if slate.pub_slate.status != SwapStatus::SETUP {
                Err("Slate must be in state SETUP to be executed".to_string())
            }
            else {
                send_msg(&mut stream, &String::from("EXECUTE"));
                if slate.pub_slate.btc.swap_type == SwapType::OFFERED {
                    // Offered value is btc, requested is grin
                    exec_phase_swap_mw(&mut slate, &mut stream, &mut btc_core, rng, &mut grin_tx, &mut grin_core, &grin_secp, btc_secp)?;
                    Ok(slate)
                }
                else {
                    // Offered value is grin, requested is btc
                    exec_phase_swap_btc(&mut slate, &mut stream, &mut btc_core, &mut grin_core, &mut grin_tx, &grin_secp)?;
                    Ok(slate)
                }
            }
        }
    }
}