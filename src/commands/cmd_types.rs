use crate::swap::protocol::setup_phase_swap_btc;
use crate::swap::protocol::setup_phase_swap_mw;
use rand::rngs::OsRng;
use crate::net::tcp::write_to_stream;
use crate::net::tcp::read_from_stream;
use crate::swap::slate::get_slate_checksum;
use crate::swap::slate::create_priv_from_pub;
use crate::grin::grin_types::MWCoin;
use crate::bitcoin::bitcoin_types::BTCInput;
use crate::swap::slate::read_slate_from_disk;
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
use crate::enums::SwapType;
use bitcoin::util::key::PrivateKey;
use rand::Rng;
use std::net::Shutdown;
use std::net::{TcpListener, TcpStream};
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::All;

pub trait Command {
    fn execute(&self, settings : Settings, rng : &mut OsRng, curve : &Secp256k1<All>) -> Result<SwapSlate, &'static str>;
}

/// The Init command will create a new Atomic Swap slate
/// 
pub struct Init {
    from : Currency,
    to : Currency,
    from_amount : u64,
    to_amount : u64,
    timeout_btc : u32,
    timeout_grin : u32
}

pub struct ImportBtc {
    swpid : u64,
    txid : String,
    vout : u16,
    value : u64,
    secret: String
}

pub struct ImportGrin {
    swpid : u64,
    commitment : String,
    blinding_factor : String,
    value : u64
}

pub struct Listen {
    swapid : u64
}

pub struct Accept {
    swapid : u64
}

pub struct Execute {
    swapid : u64
}

impl ImportBtc {
    pub fn new(swpid : u64, txid : String, vout : u16, value : u64, secret : String) -> ImportBtc {
        ImportBtc {
            swpid : swpid,
            txid : txid,
            vout : vout,
            value : value,
            secret : secret
        }
    }
}

impl ImportGrin {
    pub fn new(swpid : u64, commitment : String, blinding_factor : String, value : u64) -> ImportGrin {
        ImportGrin {
            swpid : swpid,
            commitment : commitment,
            blinding_factor : blinding_factor,
            value : value
        }
    }
}

impl Init {
    pub fn new(from : Currency, to : Currency, from_amount : u64, to_amount : u64, timeout_minutes: u32) -> Init {
        let timeout_grin : u32 = timeout_minutes / constants::GRIN_BLOCK_TIME;
        let timeout_btc : u32 = timeout_minutes / constants::BTC_BLOCK_TIME;

        Init {
            from : from,
            to: to,
            from_amount: from_amount,
            to_amount: to_amount,
            timeout_btc : timeout_btc,
            timeout_grin : timeout_grin
        }
    }
}

impl Listen {
    pub fn new(swapid : u64) -> Listen {
        Listen {
            swapid : swapid
        }
    }
}

impl Accept {
    pub fn new(swapid : u64) -> Accept {
        Accept {
            swapid : swapid
        }
    }
}

impl Execute {
    pub fn new(swapid : u64) -> Execute {
        Execute {
            swapid : swapid
        }
    }
}

impl Command for Init {
    fn execute(&self, settings : Settings, rng : &mut OsRng, curve : &Secp256k1<All>) -> Result<SwapSlate, &'static str> {
        println!("Executing init command");
        let mut rng = rand::thread_rng();

        // Create the initial Swapslate
        let id : u64 = rng.gen();
        println!("Swap id: {}", id);
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
            let prv_slate = SwapSlatePriv{
                mw : mwpriv,
                btc : btcpriv
            };

            let btc_amount = if Currency::BTC == self.from { self.from_amount } else { self.to_amount };
            let mw_amount = if Currency::GRIN == self.from { self.from_amount } else { self.to_amount };

            // Public parts set depening on from to which currency is swapped
            let btcpub = BTCPub {
                amount : btc_amount,
                timelock : self.timeout_btc,
                swap_type : if self.from == Currency::BTC { SwapType::OFFERED } else { SwapType::REQUESTED },
                stmt : None
            };
            let mwpub = MWPub {
                amount : mw_amount,
                timelock : self.timeout_grin,
                swap_type : if self.from == Currency::GRIN { SwapType::OFFERED } else { SwapType::REQUESTED }
            };
            let meta = Meta {
                server : settings.tcp_addr,
                port : settings.tcp_port
            };
            let pub_slate = SwapSlatePub {
                status : SwapStatus::INITIALIZED,
                mw : mwpub,
                btc : btcpub,
                meta : meta
            };
            Ok(SwapSlate{
                id : id,
                pub_slate : pub_slate,
                prv_slate : prv_slate
            })
        }
        else {
            Err("Swapped currency setup not supported")
        }
    }
}

impl Command for ImportBtc {
    fn execute(&self, settings: Settings, rng : &mut OsRng, curve : &Secp256k1<All>) -> Result<SwapSlate, &'static str> {
        let mut slate : SwapSlate = read_slate_from_disk(self.swpid, settings.slate_directory.clone()).expect("Failed to read SwapSlate");
        let sec_key = PrivateKey::from_wif(&self.secret).expect("Unable to parse private key, please provide in WIF format");
        slate.prv_slate.btc.inputs.push(BTCInput{
            txid : self.txid.clone(),
            vout : self.vout,
            value : self.value,
            secret : sec_key.to_wif()
        });
        Ok(slate)
    }
}

impl Command for ImportGrin {
    fn execute(&self, settings : Settings, rng : &mut OsRng, curve : &Secp256k1<All>) -> Result<SwapSlate, &'static str> {
        let mut slate : SwapSlate = read_slate_from_disk(self.swpid, settings.slate_directory.clone()).expect("Failed to read SwapSlate from file");
        slate.prv_slate.mw.inputs.push(MWCoin{
            commitment : self.commitment.clone(),
            blinding_factor : self.blinding_factor.clone(),
            value : self.value
        });
        Ok(slate)
    }
}

impl Command for Listen {
    fn execute(&self, settings : Settings, rng : &mut OsRng, curve : &Secp256k1<All>) -> Result<SwapSlate, &'static str> {

        let mut swp_slate = read_slate_from_disk(self.swapid, settings.slate_directory.clone()).expect("Failed to read SwapSlate from file");

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
            Err("Not enough value in inputs, please import more Coins")
        }
        else {    
            // Start TCP server
            let tcpaddr : String = format!("{}:{}", settings.tcp_addr, settings.tcp_port);
            println!("Starting TCP Listener on {}", tcpaddr);
            println!("Please share {}.pub.json with a interested peer. Never share your private file", self.swapid);
            let listener = TcpListener::bind(tcpaddr).unwrap(); 
            for client in listener.incoming() {
                println!("A client connected");
                let mut stream = client.unwrap();
                let msg = read_from_stream(&mut stream);
                let id = swp_slate.id.clone();
                let checksum = get_slate_checksum(id, settings.slate_directory.clone()).unwrap();
                if msg.eq_ignore_ascii_case(&checksum) {
                    println!("Swap Checksum matched");
                    // Send back OK message
                    write_to_stream(&mut stream, &String::from("OK"));
                    if swp_slate.pub_slate.btc.swap_type == SwapType::REQUESTED {
                        setup_phase_swap_btc(&mut swp_slate, &mut stream, rng, &curve)
                            .expect("Setup phase failed");
                    }
                    else {
                        setup_phase_swap_mw(&mut swp_slate, &mut stream, rng, &curve)
                            .expect("Setup phase failed");
                    }
                }
                else {
                    println!("Swap Checksum did not match, cancelling");
                }
            };
            Err("Not implemented")
        }
    } 
}

impl Command for Accept {
    fn execute(&self, settings : Settings, rng : &mut OsRng, curve : &Secp256k1<All>) -> Result<SwapSlate, &'static str> {
        let slate : SwapSlate = create_priv_from_pub(self.swapid, settings.slate_directory).expect("Unable to locate public slate file");
        println!("Created private slate file for {}", self.swapid);
        println!("Please import your inputs before starting the swap");
        Ok(slate)
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
            stream.shutdown(Shutdown::Both).expect("Failed to shutdown stream");
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