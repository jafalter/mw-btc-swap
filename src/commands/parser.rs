
// Parse arguments parsed 
use crate::commands::cmd_types::execute::Execute;
use crate::commands::cmd_types::accept::Accept;
use crate::commands::cmd_types::listen::Listen;
use crate::commands::cmd_types::import_grin::ImportGrin;
use crate::commands::cmd_types::import_btc::ImportBtc;
use crate::commands::cmd_types::command::Command;
use crate::commands::cmd_types::init::Init;
use crate::enums::Currency;
use crate::enums::parse_currency_from_string;
use crate::constants;

use std::u32;

use bitcoin::{PrivateKey, PublicKey};
use clap::{
    ArgMatches
};

use super::cmd_types::{cancel::Cancel, setup::Setup};

pub fn parse_arguments(matches: ArgMatches) -> Result<Box<dyn Command>, &'static str> {
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
        let timeout_min : u64 = timeout_arg.parse::<u64>().unwrap();

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

        Ok(Box::new(Init::new(from_currency, to_currency, from_amount, to_amount, timeout_min)))
        },
        ("import", Some(args)) => {
            match args.subcommand() {
                ("btc", Some(subargs)) => {
                    let swapid_arg = String::from(subargs.value_of("swapid").unwrap());
                    let txid = String::from(subargs.value_of("txid").unwrap());
                    let vout_arg = String::from(subargs.value_of("vout").unwrap());
                    let value_arg = String::from(subargs.value_of("value").unwrap());
                    let sk_wif = String::from(subargs.value_of("sk").unwrap());
                    let pub_script = String::from(subargs.value_of("pub_script").unwrap());
                    
                    // Parse arguments
                    let swapid : u64 = swapid_arg.parse::<u64>().unwrap();
                    let vout : u32 = vout_arg.parse::<u32>().unwrap();
                    let value : u64 = value_arg.parse::<u64>().unwrap();

                    Ok(Box::new(ImportBtc::new(swapid, txid, vout, value, sk_wif, pub_script)))
                },
                ("grin", Some(subargs)) => {
                    let swapid_arg = String::from(subargs.value_of("swapid").unwrap());
                    let commitment = String::from(subargs.value_of("commitment").unwrap());
                    let blinding_factor = String::from(subargs.value_of("blinding_factor").unwrap());
                    let value_arg = String::from(subargs.value_of("value").unwrap());

                    // Parse arguments
                    let swapid : u64 = swapid_arg.parse::<u64>().unwrap();
                    let value : u64 = value_arg.parse::<u64>().unwrap();

                    Ok(Box::new(ImportGrin::new(swapid, commitment, blinding_factor, value)))
                },
                _ => Err ("Invalid subcommand for import supplied")
            }
        },
        ("listen", Some(args)) => {
            let swapid_arg = String::from(args.value_of("swapid").unwrap());

            let swapid : u64 = swapid_arg.parse::<u64>().unwrap();

            Ok(Box::new(Listen::new(swapid)))
        },
        ("accept", Some(args)) => {
            let swapid_arg = String::from(args.value_of("swapid").unwrap());

            let swapid : u64 = swapid_arg.parse::<u64>().unwrap();

            Ok(Box::new(Accept::new(swapid)))
        },
        ("setup", Some(args)) => {
            let swapid_arg = String::from(args.value_of("swapid").unwrap());

            let swapid : u64 = swapid_arg.parse::<u64>().unwrap();

            Ok(Box::new(Setup::new(swapid)))
        },
        ("cancel", Some(args)) => {
            let swapid_arg = String::from(args.value_of("swapid").unwrap());

            let swapid : u64 = swapid_arg.parse::<u64>().unwrap();

            Ok(Box::new(Cancel::new(swapid)))
        },
        ("execute", Some(args)) => {
            let swapid_arg = String::from(args.value_of("swapid").unwrap());

            let swapid : u64 = swapid_arg.parse::<u64>().unwrap();

            Ok(Box::new(Execute::new(swapid)))
        },
        _ => Err("Invalid command supplied")
    }
}