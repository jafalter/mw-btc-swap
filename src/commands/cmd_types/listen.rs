use crate::{bitcoin::bitcoin_core::BitcoinCore, grin::{grin_core::GrinCore, grin_tx::GrinTx}, net::http::RequestFactory, swap::{protocol::setup_phase_swap_mw, slate}};
use crate::swap::protocol::setup_phase_swap_btc;
use crate::net::tcp::send_msg;
use crate::swap::slate::get_slate_checksum;
use crate::net::tcp::receive_msg;
use crate::swap::slate::read_slate_from_disk;
use rand::rngs::OsRng;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::All;
use crate::SwapSlate;
use crate::Settings;
use crate::enums::SwapType;
use crate::enums::Currency;
use std::net::{TcpListener, TcpStream};
use grin_util::secp::Secp256k1 as GrinSecp256k1;

use crate::commands::cmd_types::command::Command;

pub struct Listen {
    swapid : u64
}

impl Listen {
    pub fn new(swapid : u64) -> Listen {
        Listen {
            swapid : swapid
        }
    }
}

impl Command for Listen {
    fn execute(&self, settings : &Settings, rng : &mut OsRng, btc_secp : &Secp256k1<All>, grin_secp : &GrinSecp256k1) -> Result<SwapSlate, String> {

        let mut swp_slate = read_slate_from_disk(self.swapid, &settings.slate_directory)
            .expect("Failed to read SwapSlate from file");

        // Check if we have enough value
        let offered_currency = if swp_slate.pub_slate.mw.swap_type == SwapType::OFFERED { Currency::GRIN } else { Currency::BTC };
        let from_amount : u64 = if offered_currency == Currency::GRIN { swp_slate.pub_slate.mw.amount } else { swp_slate.pub_slate.btc.amount };
        let mut value : u64 = 0;
        if offered_currency == Currency::GRIN {
            for inp in &swp_slate.prv_slate.mw.inputs {
                value = value + inp.value;
            }
        
        }
        else {
            // Offered Bitcoin
            for inp in &swp_slate.prv_slate.btc.inputs {
                value = value + inp.value
            }
        }
        if value < from_amount {
            Err(String::from("Not enough value in inputs, please import more Coins"))
        }
        else {    
            // Start TCP server
            let tcpaddr : String = format!("{}:{}", settings.tcp_addr, settings.tcp_port);
            println!("Starting TCP Listener on {}", tcpaddr);
            println!("Please share {}.pub.json with a interested peer. Never share your private file", self.swapid);
            let mut btc_core = BitcoinCore::new(settings.btc.clone(), RequestFactory::new(None));
            let mut grin_core = GrinCore::new(settings.grin.clone(), RequestFactory::new(None));
            let mut grin_tx = GrinTx::new(settings.grin.clone(), RequestFactory::new(None));
            let listener = TcpListener::bind(tcpaddr).unwrap(); 
            for client in listener.incoming() {
                println!("A client connected");
                let mut stream = client.unwrap();
                let msg = receive_msg(&mut stream);
                let id = swp_slate.id.clone();
                let checksum = get_slate_checksum(id, &settings.slate_directory).unwrap();
                println!("Calculated slate checksum {}", checksum);

                if msg.eq_ignore_ascii_case(&checksum) {
                    println!("Swap Checksum matched");
                    // Send back OK message
                    send_msg(&mut stream, &String::from("OK"));
                    if swp_slate.pub_slate.btc.swap_type == SwapType::OFFERED {
                        setup_phase_swap_btc(&mut swp_slate, &mut stream, rng, &btc_secp, &mut grin_core, &mut btc_core, &mut grin_tx)?;
                        setup_phase_swap_btc(&mut swp_slate, &mut stream, rng, btc_secp, &mut grin_core, &mut btc_core, &mut grin_tx)?;
                        break;
                    }
                    else {
                        setup_phase_swap_mw(&mut swp_slate, &mut stream, rng, &btc_secp, &mut grin_core, &mut btc_core, &mut grin_tx)?;
                        setup_phase_swap_btc(&mut swp_slate, &mut stream, rng, btc_secp, &mut grin_core, &mut btc_core, &mut grin_tx)?;
                        break;
                    }
                }
                else {
                    println!("Swap Checksum did not match, cancelling");
                    send_msg(&mut stream, &String::from("FAULT"));
                }
            };
            Ok(swp_slate)
        }
    } 
}