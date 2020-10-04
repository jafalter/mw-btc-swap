use std::fs;
use std::env;

mod settings;
mod commands;
mod enums;
mod swap;
mod constants;

use clap::{
    Arg,
    App,
    SubCommand
};

use commands::cmd_types::Command;

fn usage() {
    println!("usage: offer|accept|redeem options");
}

fn main() {
    let contents = fs::read_to_string("config/settings.json")
        .expect("Something went wrong reading the settings file");

    let settings = settings::Settings::parse_json_string(&contents);
    println!("BTC Client: {}, Grin Client: {}", settings.btc_node_url, settings.mw_node_url);
    
    let matches = App::new("Grin Bitcoin Swaps")
                        .version("1.0")
                        .author("Jakob Abfalter <jakobabfalter@gmail.com>")
                        .subcommand(SubCommand::with_name("offer")
                        .about("Create a new atomic swap offering")
                        .arg(Arg::with_name("from-currency")
                            .long("from-currency")
                            .short("fc")
                            .required(true)
                            .takes_value(true)
                        )
                        .arg(Arg::with_name("to-currency")
                            .long("to-currency")
                            .short("tc")
                            .required(true)
                            .takes_value(true)
                        )
                        .arg(Arg::with_name("from-amount")
                            .long("from-amount")
                            .short("fa")
                            .required(true)
                            .takes_value(true)
                        )
                        .arg(Arg::with_name("to-amount")
                            .long("to-amount")
                            .short("ta")
                            .required(true)
                            .takes_value(true)
                        )
                        .arg(Arg::with_name("timeout")
                            .long("timeout")
                            .short("t")
                            .required(true)
                            .takes_value(true)
                        )
                    ).get_matches();

    let args: Vec<String> = env::args().collect();

    if args.len() <= 1 {
        usage();
    }
    else {
        let cmd = commands::parser::parse_arguments(matches)
            .expect("Failed to parse command line arguments");
        let state = cmd.execute()
            .expect("Command execution failed");

    }
}
