extern crate rustc_version;
use rustc_version::{version_meta, Channel};

fn main() {
    // Set cfg flags depending on release channel
    if let Channel::Nightly = version_meta().unwrap().channel {
        return println!("cargo:rustc-cfg=nightly");
    }

    panic!("stable build is not supported now!");
}
