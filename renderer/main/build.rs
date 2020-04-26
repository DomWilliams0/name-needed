use std::env::var;

fn main() {
    if var("TRAVIS_RUST_VERSION").is_ok() {
        let sfml_install = var("SFML_INSTALL").expect("missing SFML_INSTALL var"); // set in travis.yml
        println!("cargo:rustc-link-search={}/usr/local/lib", sfml_install);

        for lib in &["system", "window", "graphics" /* "audio", "network"*/] {
            println!("cargo:rustc-link-lib=csfml-{}", lib);
        }
    }
}
