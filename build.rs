use sha2::{Digest, Sha256};
use std::{env, fs, path::Path, process::Command};

#[path = "native/compiler.rs"]
mod compiler;

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
    println!("cargo:rerun-if-changed=native/compiler.rs");
    println!("cargo:rerun-if-env-changed=PATH");
    println!("cargo:rerun-if-changed=src");
    for vendor in ["vendor/skani", "vendor/partitions"] {
        println!("cargo:rerun-if-changed={vendor}/src");
        println!("cargo:rerun-if-changed={vendor}/Cargo.toml");
    }
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");
    let mut rust_hash = Sha256::new();
    hash_tree(Path::new("src"), &mut rust_hash);
    // Hash source and manifests, excluding generated vendor lockfiles/target directories.
    for vendor in ["vendor/skani", "vendor/partitions"] {
        hash_tree(&Path::new(vendor).join("src"), &mut rust_hash);
        rust_hash.update(fs::read(Path::new(vendor).join("Cargo.toml")).unwrap());
    }
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
    let host = env::var("HOST").unwrap();
    for key in compiler::override_keys(&host, &target) {
        println!("cargo:rerun-if-env-changed={key}");
    }
    let explicit = compiler::explicit_override(&host, &target, |key| env::var_os(key));
    let discover = compiler::needs_discovery(&host, &target, explicit.is_some())
        .unwrap_or_else(|error| panic!("{error}"));
    let mut build = cc::Build::new();
    if discover {
        let prefixes = compiler::homebrew_prefixes();
        for prefix in &prefixes {
            println!("cargo:rerun-if-changed={}", prefix.join("bin").display());
        }
        let path = compiler::discover(&prefixes, compiler::is_gnu).expect(
            "Native macOS ANIm requires Homebrew GNU C++ with C++17 support. Install Homebrew gcc before building, or set CXX to an installed GNU g++ compiler",
        );
        println!(
            "cargo:warning=ANIm selected Homebrew GNU C++: {}",
            path.display()
        );
        build.compiler(path);
    }
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
    let selected_by = explicit.as_deref().unwrap_or(if discover {
        "Homebrew discovery"
    } else {
        "cc-rs default"
    });
    let compiler = build.try_get_compiler().unwrap_or_else(|error| {
        panic!("Invalid GNU C++ compiler selected by {selected_by}: {error}")
    });
    assert!(compiler.is_like_gnu() && !compiler.is_like_clang() && compiler::is_gnu(compiler.path()),
        "Pinned MUMmer4 requires GNU C++ with C++17 support; compiler selected by {selected_by} at {} is not GNU C++. Explicit overrides are never replaced", compiler.path().display());
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
    hash.update(fs::read("native/compiler.rs").unwrap());
    hash.update(compiler.path().as_os_str().as_encoded_bytes());
    hash.update(host.as_bytes());
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
