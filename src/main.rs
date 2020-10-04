use std::fs;
use std::env;

mod settings;
mod commands;
mod enums;
mod swap;

use commands::cmd_types::Command;

fn usage() {
    println!("usage: offer|accept|redeem options");
}

fn main() {
    let contents = fs::read_to_string("config/settings.json")
        .expect("Something went wrong reading the settings file");

    let settings = settings::Settings::parse_json_string(&contents);
    println!("BTC Client: {}, Grin Client: {}", settings.btc_node_url, settings.mw_node_url);
    
    let args: Vec<String> = env::args().collect();

    if args.len() <= 1 {
        usage();
    }
    else {
        let cmd = commands::parser::parse_arguments(args).unwrap();
        let state = cmd.execute().unwrap();
    }
}
