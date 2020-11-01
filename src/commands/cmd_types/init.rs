use crate::constants::GRIN_BLOCK_TIME;
use crate::enums::Currency;
use rand::rngs::OsRng;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::secp256k1::All;
use crate::SwapSlate;
use crate::Settings;
use rand::Rng;
use crate::commands::cmd_types::command::Command;
use crate::swap::swap_types::Meta;
use crate::swap::swap_types::MWPub;
use crate::enums::SwapStatus;
use crate::enums::SwapType;
use crate::swap::swap_types::SwapSlatePub;
use crate::swap::swap_types::SwapSlatePriv;
use crate::swap::swap_types::MWPriv;
use crate::swap::swap_types::BTCPriv;
use crate::swap::swap_types::BTCPub;

/// The Init command will create a new Atomic Swap slate 
pub struct Init {
    from : Currency,
    to : Currency,
    from_amount : u64,
    to_amount : u64,
    timeout_btc : u32,
    timeout_grin : u32
}

impl Init {
    pub fn new(from : Currency, to : Currency, from_amount : u64, to_amount : u64, timeout_minutes: u32) -> Init {
        let timeout_grin : u32 = timeout_minutes / GRIN_BLOCK_TIME;
        let timeout_btc : u32 = timeout_minutes / GRIN_BLOCK_TIME;

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