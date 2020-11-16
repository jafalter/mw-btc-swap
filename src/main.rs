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
mod net;
mod util;

use clap::{
    Arg,
    App,
    SubCommand
};

use settings::Settings;

fn usage() {
    println!("usage: init|offer|accept|redeem options");
}

/// Setting variables can be overwritten with environment variables
/// these are read in this function.
/// to overwrite a variable pass it into the ENV like
/// SETTINGS_{VAR_NAME_UPPERCASE}=value
/// for example:
/// SETTINGS_TCP_ADDR=alice
fn overwrite_settings_with_env(settings : &Settings) -> Settings {
    let mw_node_url = env::var("SETTINGS_MW_NODE_URL").unwrap_or(settings.mw_node_url.clone());
    let tcp_addr = env::var("SETTINGS_TCP_ADDR").unwrap_or(settings.tcp_addr.clone());
    let tcp_port = env::var("SETTINGS_TCP_PORT").unwrap_or(settings.tcp_port.clone());
    let slate_directory = env::var("SETTINGS_SLATE_DIRECTORY").unwrap_or(settings.slate_directory.clone());

    Settings{
        btc : settings.btc.clone(),
        mw_node_url : mw_node_url,
        tcp_addr : tcp_addr,
        tcp_port : tcp_port,
        slate_directory : slate_directory
    }
}

fn main() {
    let contents = fs::read_to_string("config/settings.json")
        .expect("Something went wrong reading the settings file");

    let read_settings = settings::Settings::parse_json_string(&contents);
    let settings = overwrite_settings_with_env(&read_settings);

    // Initilize RNG
    let mut rng = util::get_os_rng();
    // Initialize curve
    let curve = util::get_secp256k1_curve();
    
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
                                .arg(Arg::with_name("pub_key")
                                    .long("pub_key")
                                    .required(true)
                                    .takes_value(true)
                                )
                                .arg(Arg::with_name("pub_script")
                                    .long("pub_script")
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
                        .subcommand(SubCommand::with_name("accept")
                            .arg(Arg::with_name("swapid")
                                .long("swapid")
                                .required(true)
                                .takes_value(true)
                            )
                        )
                        .subcommand(SubCommand::with_name("execute")
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
        let cmd = commands::parser::parse_arguments(matches)
            .expect("Failed to parse command line arguments");
        let slate : SwapSlate = cmd.execute(&settings, &mut rng, &curve)
            .expect("Command execution failed");
        
        swap::slate::write_slate_to_disk(&slate, &settings.slate_directory, true, true);
    }
}
