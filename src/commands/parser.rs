
// Parse arguments parsed 
use crate::commands::cmd_types::Command;
use crate::commands::cmd_types::Offer;
use crate::enums::Currency;

pub fn parse_arguments(args: Vec<String>) -> Result<impl Command, &'static str> {
    let cmd : String = args[1].to_uppercase();
    println!("Command parsed {}", cmd);
    match cmd.as_str() {
        "OFFER" => {
            Ok(Offer::new(Currency::BTC, Currency::GRIN, 0, 0, 0))
        },
        _ => {
            Err("Unsupported Command provided")
        }
    }
}