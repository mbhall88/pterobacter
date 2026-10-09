use std::{env, path::PathBuf, process::Command};

fn main() {
    let source = env::var("MUMMER4_SOURCE").expect("MUMMER4_SOURCE: pinned source tree");
    let native = env::var("MUMMER4_BUILD").expect("MUMMER4_BUILD: fresh static CLI build");
    for name in ["MUMMER4_SOURCE", "MUMMER4_BUILD"] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    for path in [&source, &native, "bridge.cc", "build_native.py"] {
        println!("cargo:rerun-if-changed={path}");
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    assert!(
        Command::new("python3")
            .args(["build_native.py", &source, &native])
            .arg(&out)
            .status()
            .unwrap()
            .success()
    );
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-search=native={native}/.libs");
    println!("cargo:rustc-link-lib=static=mummer4_bridge");
    println!("cargo:rustc-link-lib=static=umdmummer");
    println!("cargo:rustc-link-lib=stdc++");
    println!("cargo:rustc-link-lib=pthread");
    println!("cargo:rustc-link-lib=m");
    // The pinned static CLI build contains non-PIC native objects. This Linux
    // feasibility executable uses a non-PIE link; production must build PIC.
    println!("cargo:rustc-link-arg=-no-pie");
}
