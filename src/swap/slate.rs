use std::fs::File;
use std::fs;
use std::path::Path;
use crate::SwapSlate;

pub fn write_slate_to_disk(slate : SwapSlate, directory : String, wrt_priv: bool, wrt_pub: bool) {
    let pv_slate_path = format!("{}/{}.priv.json", directory, slate.id);
    let pb_slate_path = format!("{}/{}.pub.json", directory, slate.id);

    let pv_exists = Path::new(&pv_slate_path).exists();
    let pb_exists = Path::new(&pb_slate_path).exists();

    if !pv_exists {
        File::create(&pv_slate_path).expect("Unable to create private slate file");
    }
    if !pb_exists {
        File::create(&pb_slate_path).expect("Unable to creat public slate file");
    }

    let ser_pub_slate = "some data";
    let ser_pub_slate = "more data";

    if wrt_priv {
        println!("Writing private slate file to {}", pv_slate_path);
        fs::write(pv_slate_path, ser_pub_slate).expect("Unable to write private slate file");
    }
    if wrt_pub {
        println!("Writing public slate file to {}", pb_slate_path);
        fs::write(pb_slate_path, ser_pub_slate).expect("Unable to write public slate file");
    }
}