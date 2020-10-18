use crate::swap::swap_types::SwapSlatePriv;
use crate::swap::swap_types::SwapSlatePub;
use std::fs::File;
use std::fs;
use std::path::Path;
use crate::SwapSlate;

/// Write Atomic Swap slate into files on disk
/// 
/// # Arguments 
/// 
/// * `slate` - The Atomic Swap slate struct containing both private and public slate state
/// * `directory` - The directory in which the slate files are stored (can be configured in settings.json)
/// * `wrt_priv` - If the function should write the private file
/// * `wrt_pub` - If the function should write the public file
pub fn write_slate_to_disk(slate : SwapSlate, directory : String, wrt_priv: bool, wrt_pub: bool) {
    let pv_slate_path = get_slate_path(slate.id, directory.clone(), false);
    let pb_slate_path = get_slate_path(slate.id, directory.clone(), true);

    let pv_exists = Path::new(&pv_slate_path).exists();
    let pb_exists = Path::new(&pb_slate_path).exists();

    if !pv_exists {
        File::create(&pv_slate_path).expect("Unable to create private slate file");
    }
    if !pb_exists {
        File::create(&pb_slate_path).expect("Unable to creat public slate file");
    }

    let ser_prv_slate = serde_json::to_string(&slate.prv_slate).expect("Failed to serialize private slate data");
    let ser_pub_slate = serde_json::to_string(&slate.pub_slate).expect("Failed to serialize public slate data");

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
pub fn read_slate_from_disk(id : u64, directory : String) -> Result<SwapSlate, &'static str> {
    let pv_slate_path = get_slate_path(id, directory.clone(), false);
    let pb_slate_path = get_slate_path(id, directory.clone(), true);

    if Path::new(&pv_slate_path).exists() == false || Path::new(&pb_slate_path).exists() == false {
        Err("Unable to read slate files, as the files don't exist")
    }
    else {
        let pub_contents = fs::read_to_string(pb_slate_path).expect("Error during reading of pub file");
        let prv_contents = fs::read_to_string(pv_slate_path).expect("Error during readong of prv file");
        let pub_slate : SwapSlatePub = serde_json::from_str(&pub_contents).unwrap();
        let prv_slate : SwapSlatePriv = serde_json::from_str(&prv_contents).unwrap();
        Ok(SwapSlate {
            id : id,
            pub_slate : pub_slate,
            prv_slate : prv_slate
        })
    }
}

fn get_slate_path(id : u64, directory : String, public : bool) -> String {
    if public {
        format!("{}/{}.pub.json", directory, id)
    }
    else {
        format!("{}/{}.prv.json", directory, id)
    }
}