use crate::swap::swap_types::SwapSlate;
use std::fs;
use std::env;

mod settings;
mod commands;
mod enums;
mod swap;
mod constants;
mod grin;
mod bitcoin;

use clap::{
    Arg,
    App,
    SubCommand
};

fn usage() {
    println!("usage: init|offer|accept|redeem options");
}

fn main() {
    let contents = fs::read_to_string("config/settings.json")
        .expect("Something went wrong reading the settings file");

    let settings = settings::Settings::parse_json_string(&contents);
    println!("BTC Client: {}, Grin Client: {}", settings.btc_node_url, settings.mw_node_url);
    
    let matches = App::new("Grin Bitcoin Swaps")
                        .version("1.0")
                        .author("Jakob Abfalter <jakobabfalter@gmail.com>")
                        .subcommand(SubCommand::with_name("init")
                            .about("Create a new atomic swap slate which can then be offered")
                            .arg(Arg::with_name("from-currency")
                                .long("from-currency")
                                .required(true)
                                .takes_value(true)
                            )
                            .arg(Arg::with_name("to-currency")
                                .long("to-currency")
                                .required(true)
                                .takes_value(true)
                            )
                            .arg(Arg::with_name("from-amount")
                                .long("from-amount")
                                .required(true)
                                .takes_value(true)
                            )
                            .arg(Arg::with_name("to-amount")
                                .long("to-amount")
                                .required(true)
                                .takes_value(true)
                            )
                            .arg(Arg::with_name("timeout")
                                .long("timeout")
                                .required(true)
                                .takes_value(true)
                            )
                        )
                        .subcommand(SubCommand::with_name("import")
                            .subcommand(SubCommand::with_name("btc")
                                .arg(Arg::with_name("swapid")
                                    .long("swapid")
                                    .required(true)
                                    .takes_value(true)
                                )
                                .arg(Arg::with_name("txid")
                                    .long("txid")
                                    .required(true)
                                    .takes_value(true)
                                )
                                .arg(Arg::with_name("vout")
                                    .long("vout")
                                    .required(true)
                                    .takes_value(true)
                                )
                                .arg(Arg::with_name("value")
                                    .long("value")
                                    .required(true)
                                    .takes_value(true)
                                )
                                .arg(Arg::with_name("secret")
                                    .long("secret")
                                    .required(true)
                                    .takes_value(true)
                                )
                            )
                            .subcommand(SubCommand::with_name("grin")
                                .arg(Arg::with_name("swapid")
                                    .long("swapid")
                                    .required(true)
                                    .takes_value(true)
                                )
                                .arg(Arg::with_name("commitment")
                                    .long("commitment")
                                    .required(true)
                                    .takes_value(true)
                                )
                                .arg(Arg::with_name("blinding_factor")
                                    .long("blinding_factor")
                                    .required(true)
                                    .takes_value(true)
                                )
                                .arg(Arg::with_name("value")
                                    .long("value")
                                    .required(true)
                                    .takes_value(true)
                                )
                            )
                        )
                        .subcommand(SubCommand::with_name("listen")
                            .arg(Arg::with_name("swapid")
                                .long("swapid")
                                .required(true)
                                .takes_value(true)
                            )
                        )
                        .get_matches();

    let args: Vec<String> = env::args().collect();

    if args.len() <= 1 {
        usage();
    }
    else {
        let slate_dir = settings.slate_directory.clone();
        let cmd = commands::parser::parse_arguments(matches)
            .expect("Failed to parse command line arguments");
        let slate : SwapSlate = cmd.execute(settings)
            .expect("Command execution failed");
        
        swap::slate::write_slate_to_disk(slate, slate_dir, true, true);
    }
}
