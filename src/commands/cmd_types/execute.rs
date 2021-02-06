use crate::{bitcoin::bitcoin_core::BitcoinCore, grin::{grin_core::GrinCore, grin_tx::GrinTx}, net::http::RequestFactory, swap::{protocol::exec_phase_swap_mw, slate::{get_slate_checksum, write_slate_to_disk}}};
use crate::swap::slate::read_slate_from_disk;
use rand::rngs::OsRng;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::All;
use crate::SwapSlate;
use crate::Settings;
use crate::commands::cmd_types::command::Command;
use std::net::{TcpStream};
use crate::net::tcp::write_to_stream;
use crate::net::tcp::read_from_stream;
use std::net::Shutdown;
use crate::enums::SwapType;
use crate::swap::protocol::setup_phase_swap_btc;
use crate::swap::protocol::setup_phase_swap_mw;
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
        write_to_stream(&mut stream, &checksum);
        let resp = read_from_stream(&mut stream);
        if resp.eq_ignore_ascii_case("OK") == false {
            stream.shutdown(Shutdown::Both)
                .expect("Failed to shutdown stream");
            Err(String::from("Checksums didn't match cancelled swap"))
        }
        else {
            if slate.pub_slate.btc.swap_type == SwapType::OFFERED {
                // Offered value is btc, requested is grin
                setup_phase_swap_mw(&mut slate, &mut stream, rng, &btc_secp, &mut grin_core, &mut btc_core, &mut grin_tx)?;
                write_slate_to_disk(&slate, &settings.slate_directory, true, true);
                exec_phase_swap_mw(&mut slate, &mut stream, &mut btc_core, rng, &mut grin_tx, &grin_secp, btc_secp)?;
                Ok(slate)
            }
            else {
                // Offered value is grin, requested is btc
                setup_phase_swap_btc(&mut slate, &mut stream, rng, &btc_secp, &mut grin_core, &mut btc_core, &mut grin_tx)?;
                write_slate_to_disk(&slate, &settings.slate_directory, true, true);
                exec_phase_swap_mw(&mut slate, &mut stream, &mut btc_core, rng, &mut grin_tx, &grin_secp, btc_secp)?;
                Ok(slate)
            }
        }
    }
}