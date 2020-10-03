use crate::swap::swap_types::SwapState;
use crate::enums::Currency;

pub trait Command {
    fn execute(&self) -> Result<SwapState, String>;
}

struct Offer {
    from : Currency,
    to : Currency,
    from_amount : u32,
    to_amount : u32,
    timeout_btc : u32,
    timoeut_grin : u32,
    exchange_rate : f32
}

impl Offer {
    fn new(from : Currency, to : Currency, from_amount : u32, to_amount : u32, timeout_minutes: u32) -> Offer {
        let mut exchange_rate = 1.0;
        if from_amount > to_amount {
            exchange_rate = (from_amount as f32) / (to_amount as f32) ;
        }
        else if to_amount > from_amount {
            exchange_rate = (to_amount as f32) / (from_amount as f32);
        }

        Offer {
            from : from,
            to: to,
            from_amount: from_amount,
            to_amount: to_amount,
            timeout_btc : 0, // TODO correct
            timoeut_grin : 0,  // TODO correct
            exchange_rate : exchange_rate
        }
    }
}

impl Command for Offer {
    fn execute(&self) -> Result<SwapState, String> {

    }
}