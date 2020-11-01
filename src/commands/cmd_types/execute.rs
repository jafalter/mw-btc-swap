use crate::swap::slate::get_slate_checksum;
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
    fn execute(&self, settings : Settings, rng : &mut OsRng, curve : &Secp256k1<All>) -> Result<SwapSlate, &'static str> {
        let mut slate : SwapSlate = read_slate_from_disk(self.swapid, settings.slate_directory.clone()).expect("Unable to read slate files from disk");
        let mut stream : TcpStream = TcpStream::connect(format!("{}:{}", slate.pub_slate.meta.server, slate.pub_slate.meta.port))
            .expect("Failed to connect to peer via TCP");
        // first message exchanged is a hash of the pub slate file
        println!("Connected to peer");
        let checksum = get_slate_checksum(slate.id, settings.slate_directory.clone()).unwrap();
        write_to_stream(&mut stream, &checksum);
        let resp = read_from_stream(&mut stream);
        if resp.eq_ignore_ascii_case("OK") == false {
            stream.shutdown(Shutdown::Both)
                .expect("Failed to shutdown stream");
            Err("Checksums didn't match cancelled swap")
        }
        else {
            if slate.pub_slate.btc.swap_type == SwapType::OFFERED {
                // Offered value is btc, requested is grin
                setup_phase_swap_mw(&mut slate, &mut stream, rng, &curve).expect("Setup phase failed");
                Err("Not implemented")
            }
            else {
                // Offerec value is grin, requested is btc
                setup_phase_swap_btc(&mut slate, &mut stream, rng, &curve).expect("Setup phase failed");
                Err("Not implemented")
            }
        }
    }
}