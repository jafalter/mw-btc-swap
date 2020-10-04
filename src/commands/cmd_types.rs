use crate::swap::swap_types::SwapState;
use crate::enums::Currency;
use crate::enums::SwapStatus;
use crate::constants;

pub trait Command {
    fn execute(&self) -> Result<SwapState, &'static str>;
}

pub struct Offer {
    from : Currency,
    to : Currency,
    from_amount : u64,
    to_amount : u64,
    timeout_btc : u32,
    timeout_grin : u32,
    exchange_rate : f32
}

impl Offer {
    pub fn new(from : Currency, to : Currency, from_amount : u64, to_amount : u64, timeout_minutes: u32) -> Offer {
        let mut exchange_rate = 1.0;
        if from_amount > to_amount {
            exchange_rate = (from_amount as f32) / (to_amount as f32) ;
        }
        else if to_amount > from_amount {
            exchange_rate = (to_amount as f32) / (from_amount as f32);
        }
        let timeout_grin : u32 = timeout_minutes / constants::GRIN_BLOCK_TIME;
        let timeout_btc : u32 = timeout_minutes / constants::BTC_BLOCK_TIME;

        Offer {
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

impl Command for Offer {
    fn execute(&self) -> Result<SwapState, &'static str> {
        println!("Executing offer command");
        // Start TCP server
        // Output a token with which a peer can connect
        Ok(SwapState{
            status : SwapStatus::INITIALIZED
        })
    }
}