use sha2::{Digest, Sha256};
use std::{env, fs, path::Path, process::Command};

fn hash_tree(path: &Path, hash: &mut Sha256) {
    let mut entries: Vec<_> = fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            hash_tree(&entry, hash)
        } else {
            hash.update(entry.to_str().unwrap().as_bytes());
            hash.update([0]);
            hash.update(fs::read(&entry).unwrap());
            hash.update([0]);
        }
    }
}
fn main() {
    println!("cargo:rerun-if-changed=vendor/mummer4");
    println!("cargo:rerun-if-changed=native/anim.cc");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");
    let mut rust_hash = Sha256::new();
    hash_tree(Path::new("src"), &mut rust_hash);
    rust_hash.update(fs::read("Cargo.toml").unwrap());
    rust_hash.update(fs::read("Cargo.lock").unwrap());
    println!(
        "cargo:rustc-env=TERO_ANIM_SOURCE_SHA256={:x}",
        rust_hash.finalize()
    );
    let target = env::var("TARGET").unwrap();
    assert!(
        target.contains("linux") || target.contains("apple-darwin"),
        "ANIm process containment currently requires Linux or macOS"
    );
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .cpp_link_stdlib("stdc++")
        .std("c++17")
        .opt_level(3)
        .debug(false)
        .pic(true)
        .define("NDEBUG", None)
        .flag("-pthread")
        .flag("-fno-openmp")
        .include("vendor/mummer4/include")
        .warnings(false);
    let compiler = build.get_compiler();
    assert!(compiler.is_like_gnu() && !compiler.is_like_clang(), "Pinned MUMmer4 requires GCC: set CXX to a supported g++ compiler; macOS native parity remains a validation gate");
    let version = Command::new(compiler.path())
        .arg("--version")
        .output()
        .expect("C++ compiler version");
    assert!(version.status.success());
    let version = String::from_utf8(version.stdout)
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .to_owned();
    if target.contains("apple-darwin") {
        // Homebrew GCC uses libstdc++, whereas cc's macOS default is libc++.
        let library = Command::new(compiler.path())
            .arg("-print-file-name=libstdc++.dylib")
            .output()
            .expect("GNU C++ runtime location");
        assert!(library.status.success());
        let path = String::from_utf8(library.stdout).unwrap();
        let directory = Path::new(path.trim())
            .parent()
            .expect("GNU C++ runtime directory");
        assert!(directory.is_dir(), "cannot locate GNU C++ runtime");
        println!("cargo:rustc-link-search=native={}", directory.display());
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", directory.display());
    }
    let mut hash = Sha256::new();
    hash_tree(Path::new("vendor/mummer4"), &mut hash);
    hash.update(fs::read("native/anim.cc").unwrap());
    hash.update(fs::read("build.rs").unwrap());
    hash.update(version.as_bytes());
    hash.update(target.as_bytes());
    hash.update(format!("{:?}", compiler.to_command().get_args()).as_bytes());
    println!(
        "cargo:rustc-env=TERO_ANIM_BUILD_SHA256={:x}",
        hash.finalize()
    );
    println!("cargo:rustc-env=TERO_ANIM_COMPILER={version}");
    println!("cargo:rustc-env=TERO_ANIM_TARGET={target}");
    for file in [
        "src/essaMEM/sparseSA.cpp",
        "src/essaMEM/sssort_compact.cc",
        "src/tigr/mgaps.cc",
        "src/tigr/postnuc.cc",
        "src/tigr/sw_align.cc",
        "src/tigr/tigrinc.cc",
        "src/umd/nucmer.cc",
        "src/tigr/delta.cc",
    ] {
        build.file(format!("vendor/mummer4/{file}"));
    }
    build.file("native/anim.cc").compile("tero_anim");
    println!("cargo:rustc-link-lib=pthread");
}
