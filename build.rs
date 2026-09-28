#[rustversion::nightly]
const NIGHTLY: bool = true;

#[rustversion::not(nightly)]
const NIGHTLY: bool = false;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/detail/asm/asm_ppc64le_elf.S");
    println!("cargo:rustc-check-cfg=cfg(nightly)");
    println!("cargo:rustc-check-cfg=cfg(ppc64_global_asm)");
    if NIGHTLY {
        println!("cargo:rustc-cfg=nightly");
    }
    if std::env::var("CARGO_CFG_TARGET_ARCH").unwrap() == "powerpc64" {
        build_powerpc_assembly();
    }
}
#[rustversion::before(1.95)]
fn build_powerpc_assembly() {
    cc::Build::new()
        .file("src/detail/asm/asm_ppc64le_elf.S")
        .compile("ppc64le-asm-lib");
}

#[rustversion::since(1.95)]
fn build_powerpc_assembly() {
    // Convert register aliases to numbers; preprocessor directives remain comments.
    let registers = regex_lite::Regex::new(r"\b[rfv]([0-9]+)\b").unwrap();
    let assembly = registers.replace_all(
        include_str!("src/detail/asm/asm_ppc64le_elf.S"),
        "$1",
    );
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(output.join("asm_ppc64le_elf.S"), assembly.as_bytes()).unwrap();
    println!("cargo:rustc-cfg=ppc64_global_asm");
}
