use std::path::PathBuf;

use bindgen;
use cmake;

fn main() {
    // build libbulletc
    let dst = cmake::Config::new("bulletc").build();
    println!("cargo:rustc-link-search=native={}", dst.display());
    println!("cargo:rustc-link-lib=dylib=bulletc");
    println!("cargo:rustc-flags=-l dylib=stdc++");

    // for easy access for debugging
    std::fs::copy(dst.join("libbulletc.so"), "../libbulletc.so").expect("couldnt copy dylib");

    // bindgen
    let header = "bulletc/bulletc.hpp";
    println!("cargo:rerun-if-changed={}", header);
    println!("cargo:rerun-if-changed=bulletc/bulletc.cpp");
    let bindings = bindgen::Builder::default()
        .header(header)
        .generate()
        .expect("failed to generate bulletc bindings");

    // let out_path = PathBuf::from(::std::env::var("OUT_DIR").unwrap());
    let out_path = PathBuf::from("src");
    bindings
        .write_to_file(out_path.join("bulletc.rs"))
        .expect("failed to write bindings");
}
