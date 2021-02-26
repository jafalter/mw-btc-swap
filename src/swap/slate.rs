use crate::swap::swap_types::BTCPriv;
use crate::swap::swap_types::MWPriv;
use crate::swap::swap_types::SwapSlatePriv;
use crate::swap::swap_types::SwapSlatePub;
use std::fs::File;
use std::fs;
use std::path::Path;
use crate::SwapSlate;
use sha2::{Sha256, Digest};

/// Write Atomic Swap slate into files on disk
/// 
/// # Arguments 
/// 
/// * `slate` - The Atomic Swap slate struct containing both private and public slate state
/// * `directory` - The directory in which the slate files are stored (can be configured in settings.json)
/// * `wrt_priv` - If the function should write the private file
/// * `wrt_pub` - If the function should write the public file
pub fn write_slate_to_disk(slate : &SwapSlate, directory : &str, wrt_priv: bool, wrt_pub: bool) {
    let pv_slate_path = get_slate_path(slate.id, &directory, false);
    let pb_slate_path = get_slate_path(slate.id, &directory, true);

    let pv_exists = Path::new(&pv_slate_path).exists();
    let pb_exists = Path::new(&pb_slate_path).exists();

    if !pv_exists {
        File::create(&pv_slate_path).expect("Unable to create private slate file");
    }
    if !pb_exists {
        File::create(&pb_slate_path).expect("Unable to creat public slate file");
    }

    let ser_prv_slate = serde_json::to_string_pretty(&slate.prv_slate).expect("Failed to serialize private slate data");
    let ser_pub_slate = serde_json::to_string_pretty(&slate.pub_slate).expect("Failed to serialize public slate data");

    if wrt_priv {
        println!("Writing private slate file to {}", pv_slate_path);
        fs::write(pv_slate_path, ser_prv_slate).expect("Unable to write private slate file");
    }
    if wrt_pub {
        println!("Writing public slate file to {}", pb_slate_path);
        fs::write(pb_slate_path, ser_pub_slate).expect("Unable to write public slate file");
    }
}

/// Read Atomic Swap Slate from files stored on disk
/// 
/// # Arguments
/// 
/// * `id` the id of the Atomi Swap
/// * `directory` the directory in which the slate files are stored. (Can be configured in settings.json)
pub fn read_slate_from_disk(id : u64, directory : &str) -> Result<SwapSlate, &'static str> {
    let pv_slate_path = get_slate_path(id, &directory, false);
    let pb_slate_path = get_slate_path(id, &directory, true);

    if Path::new(&pv_slate_path).exists() == false || Path::new(&pb_slate_path).exists() == false {
        Err("Unable to read slate files, as the files don't exist")
    }
    else {
        let pub_contents = fs::read_to_string(pb_slate_path)
            .expect("Error during reading of pub file");
        let prv_contents = fs::read_to_string(pv_slate_path)
            .expect("Error during readong of prv file");
        let pub_slate : SwapSlatePub = serde_json::from_str(&pub_contents).unwrap();
        let prv_slate : SwapSlatePriv = serde_json::from_str(&prv_contents).unwrap();
        Ok(SwapSlate {
            id : id,
            pub_slate : pub_slate,
            prv_slate : prv_slate
        })
    }
}

pub fn get_slate_checksum(id : u64, directory : &str) -> Result<String, &'static str> {
    let pb_slate_path = get_slate_path(id, &directory, true);

    if Path::new(&pb_slate_path).exists() == false {
        Err("Unable to read slate files, as the files don't exist")
    }
    else {
        let pub_contents = fs::read_to_string(pb_slate_path)
            .expect("Error during reading of pub slate file");
        let mut hasher = Sha256::new();
        hasher.update(pub_contents);
        let bytes = hasher.finalize();
        Ok(format!("{:x}", bytes))
    }
}

/// Create a fresh private slate file for a swap identified by the id
/// 
/// # Arguments
/// 
/// * `id` the id of the Atomic Swap
/// * `directory` the directory in which the slate files are store. (Can be configured in settions.json)
pub fn create_priv_from_pub(id : u64, directory : &str) -> Result<SwapSlate, &'static str> {
    let pb_slate_path = get_slate_path(id, &directory, true);

    if Path::new(&pb_slate_path).exists() == false {
        Err("Unable to create private slate file, as the public file doesn't exist")
    }
    else {
        let pub_contents = fs::read_to_string(pb_slate_path).expect("Error during reading of pub file");
        let pub_slate : SwapSlatePub = serde_json::from_str(&pub_contents).unwrap();

        let mwpriv = MWPriv{
            inputs : Vec::new(),
            partial_key : 0,
            shared_coin : None,
            refund_coin : None,
            swapped_coin : None,
            change_coin : None,
            refund_tx : None
        };        
        let btcpriv = BTCPriv{
            inputs : Vec::new(),
            witness : 0,
            sk : None,
            x : None,
            r_sk : None,
            swapped : None,
            change : None,
            lock : None,
            refunded : None
        };
        let prv_slate = SwapSlatePriv{
            mw : mwpriv,
            btc : btcpriv
        };
        let slate : SwapSlate = SwapSlate {
            id : id,
            pub_slate : pub_slate,
            prv_slate : prv_slate
        };
        write_slate_to_disk(&slate, directory, true, false);

        Ok(slate)
    }
}

fn get_slate_path(id : u64, directory : &str, public : bool) -> String {
    if public {
        let dir = format!("{}/{}.pub.json", directory, id);
        dir
    }
    else {
        let dir = format!("{}/{}.prv.json", directory, id);
        dir
    }
}