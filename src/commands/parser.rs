
// Parse arguments parsed 
use crate::commands::cmd_types::Command;

pub fn parse_arguments(args: Vec<String>) -> Result<impl Command, String> {
    let cmd : String = args[0].to_string().to_uppercase();
    match cmd {
        "OFFER" => {
            println!("Offer command parsed");
        },
        "ACCEPT" => {
            println!("Accept command parsed");
        }
        "REDEEM" => {
            println!("Redeem command parsed");
        }
        _ => {
            Err("Not a valid command");
        }
    }
}