use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    // THERMAL_N657=1 selects the RAM-only physical-board map; default is the
    // QEMU mps3-an547 map.
    let src = if env::var("THERMAL_N657").is_ok() {
        &include_bytes!("memory-n657.x")[..]
    } else {
        &include_bytes!("memory-qemu.x")[..]
    };
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    File::create(out.join("memory.x")).unwrap().write_all(src).unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory-qemu.x");
    println!("cargo:rerun-if-changed=memory-n657.x");
    println!("cargo:rerun-if-env-changed=THERMAL_N657");
    println!("cargo:rerun-if-changed=build.rs");
}
