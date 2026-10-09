use std::{env, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-env-changed=MUMMER3_SOURCE");
    println!("cargo:rerun-if-changed=build_mummer.py");
    if env::var_os("CARGO_FEATURE_EMBEDDED_MUMMER3").is_none() {
        return;
    }
    assert!(
        env::var("TARGET").unwrap().contains("linux"),
        "the development embedding build uses GNU ELF tools and currently supports Linux only"
    );
    let source = env::var("MUMMER3_SOURCE")
        .expect("set MUMMER3_SOURCE to an isolated MUMmer 3.23 source copy");
    // Watch the entire supplied tree, including replacement at the same path.
    println!("cargo:rerun-if-changed={source}");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    assert!(
        Command::new("python3")
            .args(["build_mummer.py", &source])
            .arg(&out)
            .status()
            .unwrap()
            .success(),
        "native probe build failed"
    );
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=embedded_mummer3");
    println!("cargo:rustc-link-lib=stdc++");
    println!("cargo:rustc-link-lib=m");
}
