
// Parse arguments parsed 
use crate::commands::cmd_types::Command;
use crate::commands::cmd_types::Init;
use crate::enums::Currency;
use crate::enums::parse_currency_from_string;
use crate::constants;

use clap::{
    ArgMatches
};

pub fn parse_arguments(matches: ArgMatches) -> Result<impl Command, &'static str> {
    match matches.subcommand() {
        ("init", Some(args)) => {
        let from_currency_arg = String::from(args.value_of("from-currency").unwrap());
        let to_currency_arg = String::from(args.value_of("to-currency").unwrap());
        let from_amount_arg = String::from(args.value_of("from-amount").unwrap());
        let to_amount_arg = String::from(args.value_of("to-amount").unwrap());
        let timeout_arg = String::from(args.value_of("timeout").unwrap());
        
        // Parse arguments
        let from_currency = parse_currency_from_string(from_currency_arg);
        let to_currency = parse_currency_from_string(to_currency_arg);
        let from_amount : u64 = from_amount_arg.parse::<u64>().unwrap();
        let to_amount : u64 = to_amount_arg.parse::<u64>().unwrap();
        let timeout_min : u32 = timeout_arg.parse::<u32>().unwrap();

        // Validate arguments
        let from_overflow = ( from_currency == Currency::BTC && from_amount > constants::BTC_MAX_SATS ) || ( from_currency == Currency::GRIN && from_amount > constants::GRIN_MAX_NANOGRIN );
        let to_overflow = ( to_currency == Currency::BTC && to_amount > constants::BTC_MAX_SATS ) || ( to_currency == Currency::GRIN && to_amount > constants::GRIN_MAX_NANOGRIN );

        if from_overflow {
            panic!("From amount is too high!");
        }
        if  to_overflow  {
            panic!("To amount is too high!");
        }
        if timeout_min > constants::MAX_TIMEOUT {
            panic!("Timeout too high! Max timeout is 5 days");
        }

        Ok(Init::new(from_currency, to_currency, from_amount, to_amount, timeout_min))
        },
        _ => Err("Invalid command supplied")
    }
}