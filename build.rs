extern crate cc;

use std::env;
use std::path::PathBuf;

fn main() {
    // Set cfg flags depending on release channel
    if NIGHTLY {
        println!("cargo:rustc-cfg=nightly");
    }

    // for the stable build asm lib
    let target: String = env::var("TARGET").unwrap();
    let is_win_gnu = target.ends_with("windows-gnu");
    let is_win_msvc = target.ends_with("windows-msvc");
    let is_win = is_win_gnu || is_win_msvc;

    let arch = match target.split('-').next().unwrap() {
        // "arm" | "armv7" | "armv7s" => "arm",
        "arm64" | "aarch64" => "aarch64",
        // "x86" | "i386" | "i486" | "i586" | "i686" => "i386",
        // "mips" | "mipsel" => "mips32",
        // "powerpc" => "ppc32",
        // "powerpc64" => "ppc64",
        "x86_64" => "x86_64",
        _ => {
            panic!("Unsupported architecture: {target}");
        }
    };

    let abi = match arch {
        "arm" | "aarch64" => "aapcs",
        "mips32" => "o32",
        _ => {
            if is_win {
                "ms"
            } else {
                "sysv"
            }
        }
    };

    let format = if is_win {
        "pe"
    } else if target.contains("apple") {
        "macho"
    } else if target.ends_with("aix") {
        "xcoff"
    } else {
        "elf"
    };

    let (asm, ext) = if is_win_msvc {
        if arch == "arm" {
            ("armasm", "asm")
        } else {
            ("masm", "asm")
        }
    } else if is_win_gnu {
        ("gas", "asm")
    } else {
        ("gas", "S")
    };

    let mut path: PathBuf = "src/detail/asm".into();
    let mut config = cc::Build::new();

    if is_win_gnu {
        config.flag("-x").flag("assembler-with-cpp");
    }

    let file_name: [&str; 11] = ["asm", "_", arch, "_", abi, "_", format, "_", asm, ".", ext];
    let file_name = file_name.concat();

    path.push(file_name);
    println!("cargo:rerun-if-changed={}", path.display());
    config.file(path);

    // create the static asm libary
    config.compile("libasm.a");
}

#[rustversion::nightly]
const NIGHTLY: bool = true;

#[rustversion::not(nightly)]
const NIGHTLY: bool = false;
