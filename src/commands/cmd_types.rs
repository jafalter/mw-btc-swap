use crate::swap::swap_types::Meta;
use crate::swap::swap_types::MWPub;
use crate::swap::swap_types::SwapSlate;
use crate::swap::swap_types::SwapSlatePub;
use crate::swap::swap_types::SwapSlatePriv;
use crate::swap::swap_types::MWPriv;
use crate::swap::swap_types::BTCPriv;
use crate::swap::swap_types::BTCPub;
use crate::enums::Currency;
use crate::enums::SwapStatus;
use crate::constants;
use crate::settings::Settings;
use rand::Rng;
use std::net::{TcpListener, TcpStream};

pub trait Command {
    fn execute(&self, settings : Settings) -> Result<SwapSlate, &'static str>;
}

pub struct Init {
    from : Currency,
    to : Currency,
    from_amount : u64,
    to_amount : u64,
    timeout_btc : u32,
    timeout_grin : u32,
    exchange_rate : f32
}

pub struct Offer {
    slate : SwapSlate
}

impl Init {
    pub fn new(from : Currency, to : Currency, from_amount : u64, to_amount : u64, timeout_minutes: u32) -> Init {
        let mut exchange_rate = 1.0;
        if from_amount > to_amount {
            exchange_rate = (from_amount as f32) / (to_amount as f32) ;
        }
        else if to_amount > from_amount {
            exchange_rate = (to_amount as f32) / (from_amount as f32);
        }
        let timeout_grin : u32 = timeout_minutes / constants::GRIN_BLOCK_TIME;
        let timeout_btc : u32 = timeout_minutes / constants::BTC_BLOCK_TIME;

        Init {
            from : from,
            to: to,
            from_amount: from_amount,
            to_amount: to_amount,
            timeout_btc : timeout_btc,
            timeout_grin : timeout_grin,
            exchange_rate : exchange_rate
        }
    }
}

impl Command for Init {
    fn execute(&self, settings : Settings) -> Result<SwapSlate, &'static str> {
        println!("Executing init command");
        let mut rng = rand::thread_rng();

        // Create the initial Swapslate
        let id : u64 = rng.gen();
        if self.from == Currency::BTC && self.to == Currency::GRIN || self.from == Currency::GRIN && self.to == Currency::BTC {
            // Private parts are unset for now
            let mwpriv = MWPriv{
                inputs : Vec::new(),
                partial_key : 0
            };        
            let btcpriv = BTCPriv{
                inputs : Vec::new(),
                witness : 0
            };
            let privSlate = SwapSlatePriv{
                mw : mwpriv,
                btc : btcpriv
            };

            let btc_amount = if Currency::BTC == self.from { self.from_amount } else { self.to_amount };
            let mw_amount = if Currency::GRIN == self.from { self.from_amount } else { self.to_amount };

            // Public parts set depening on from to which currency is swapped
            let btcpub = BTCPub {
                amount : btc_amount,
                timelock : self.timeout_btc,
                stmt : None
            };
            let mwpub = MWPub {
                amount : mw_amount,
                timelock : self.timeout_grin
            };
            let meta = Meta {
                server : settings.tcp_addr,
                port : settings.tcp_port
            };
            let pubSlate = SwapSlatePub {
                status : SwapStatus::INITIALIZED,
                mw : mwpub,
                btc : btcpub,
                meta : meta
            };
            Ok(SwapSlate{
                id : id,
                pubSlate : pubSlate,
                privSlate : privSlate
            })
        }
        else {
            Err("Swapped currency setup not supported")
        }
    }
}

/*
impl Command for Offer {
    fn execute(&self, settings : Settings) {
        println!("Executing offer command");
        // Start TCP server
        // Output a token with which a peer can connect with
        let tcpaddr : String = format!("{}:{}", settings.tcp_addr, settings.tcp_port);
        println!("Starting TCP Listener on {}", tcpaddr);
        let listener = TcpListener::bind(tcpaddr).unwrap(); 
        for stream in listener.incoming() {
            println!("A client connected");
        }
    } 
}
*/