#[path = "../native/compiler.rs"]
mod compiler;
use std::{collections::HashMap, ffi::OsString, path::PathBuf};

#[test]
fn cc_override_precedence_matches_native_and_cross_builds() {
    for (host, target, kind) in [
        ("aarch64-apple-darwin", "aarch64-apple-darwin", "HOST"),
        ("x86_64-apple-darwin", "aarch64-apple-darwin", "TARGET"),
    ] {
        let keys = compiler::override_keys(host, target);
        assert_eq!(
            keys,
            [
                format!("CXX_{target}"),
                format!("CXX_{}", target.replace('-', "_")),
                format!("{kind}_CXX"),
                "CXX".into()
            ]
        );
        let mut values: HashMap<_, _> = keys
            .iter()
            .map(|k| (k.clone(), OsString::from("invalid-explicit-compiler")))
            .collect();
        values.insert(
            if kind == "HOST" {
                "TARGET_CXX"
            } else {
                "HOST_CXX"
            }
            .into(),
            "wrong-kind".into(),
        );
        for key in keys {
            assert_eq!(
                compiler::explicit_override(host, target, |k| values.get(k).cloned()).unwrap(),
                Some(key.clone())
            );
            assert!(!compiler::needs_discovery(host, target, true).unwrap());
            values.remove(&key);
        }
        assert_eq!(
            compiler::explicit_override(host, target, |k| values.get(k).cloned()).unwrap(),
            None
        );
    }
}

#[test]
fn native_mac_only_and_cross_builds_require_explicit_compilers() {
    assert!(
        compiler::needs_discovery("aarch64-apple-darwin", "aarch64-apple-darwin", false).unwrap()
    );
    assert!(!compiler::needs_discovery(
        "x86_64-unknown-linux-gnu",
        "x86_64-unknown-linux-gnu",
        false
    )
    .unwrap());
    for (host, target) in [
        ("x86_64-apple-darwin", "aarch64-apple-darwin"),
        ("x86_64-unknown-linux-gnu", "aarch64-apple-darwin"),
        ("aarch64-apple-darwin", "x86_64-unknown-linux-gnu"),
    ] {
        assert!(compiler::needs_discovery(host, target, false)
            .unwrap_err()
            .contains("Cross-compilation"));
        assert!(!compiler::needs_discovery(host, target, true).unwrap());
    }
}

#[test]
fn discovery_prefers_installed_prefix_and_numeric_versions_at_both_locations() {
    let dir = tempfile::tempdir().unwrap();
    let installed = dir.path().join("Cellar/gcc/15");
    let apple = dir.path().join("opt/homebrew/opt/gcc");
    let intel = dir.path().join("usr/local/opt/gcc");
    for prefix in [&installed, &apple, &intel] {
        std::fs::create_dir_all(prefix.join("bin")).unwrap();
        for name in ["g++-9", "g++-14", "g++-15", "g++-invalid", "clang++"] {
            std::fs::write(prefix.join("bin").join(name), "").unwrap();
        }
        assert_eq!(
            compiler::discover(std::slice::from_ref(prefix), |_| true),
            Some(prefix.join("bin/g++-15"))
        );
        assert_eq!(
            compiler::discover(std::slice::from_ref(prefix), |p| p.ends_with("g++-14")),
            Some(prefix.join("bin/g++-14"))
        );
    }
    assert_eq!(
        compiler::discover(&[installed.clone(), apple.clone(), intel.clone()], |_| true),
        Some(installed.join("bin/g++-15"))
    );
    assert_eq!(
        compiler::discover(&[PathBuf::from("missing"), intel.clone()], |_| true),
        Some(intel.join("bin/g++-15"))
    );
    assert_eq!(compiler::discover(&[apple, intel], |_| false), None);
    assert_eq!(
        compiler::discover(&[dir.path().join("empty")], |_| true),
        None
    );
}

#[test]
fn compiler_family_probe_rejects_missing_and_non_compilers() {
    assert!(!compiler::is_gnu(std::path::Path::new(
        "/definitely/missing/compiler"
    )));
    assert!(!compiler::is_gnu(std::path::Path::new("/bin/false")));
    // Exercise prefix querying as well as its pure candidate selection.
    let _ = compiler::homebrew_prefixes();
}

#[test]
fn blank_selected_overrides_fail_at_every_applicable_level_without_fallback() {
    for (host, target) in [
        ("x86_64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"),
        ("aarch64-apple-darwin", "aarch64-apple-darwin"),
        ("x86_64-apple-darwin", "aarch64-apple-darwin"),
    ] {
        let keys = compiler::override_keys(host, target);
        for (index, key) in keys.iter().enumerate() {
            for blank in ["", "   ", "\t\n\r", "\u{2003}"] {
                let mut values: HashMap<String, OsString> = keys[index + 1..]
                    .iter()
                    .map(|k| (k.clone(), "g++".into()))
                    .collect();
                values.insert(key.clone(), blank.into());
                let error = compiler::explicit_override(host, target, |k| values.get(k).cloned())
                    .unwrap_err();
                assert!(error.contains(&format!("override {key}:")), "{error}");
                assert!(error.contains("empty or whitespace"), "{error}");
            }
        }
    }
}

#[test]
fn nonblank_overrides_keep_wrapper_arguments_and_ignore_lower_blank_values() {
    let host = "x86_64-unknown-linux-gnu";
    let keys = compiler::override_keys(host, host);
    for value in ["g++", " g++ -pthread ", " sccache g++ -pthread "] {
        let mut values: HashMap<String, OsString> =
            keys.iter().map(|key| (key.clone(), "   ".into())).collect();
        values.insert(keys[0].clone(), value.into());
        assert_eq!(
            compiler::explicit_override(host, host, |k| values.get(k).cloned()).unwrap(),
            Some(keys[0].clone())
        );
        assert_eq!(values[&keys[0]], OsString::from(value));
    }
}

#[test]
fn cpp_runtime_selection_preserves_defaults_and_rejects_unsupported_overrides() {
    for target in [
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
    ] {
        assert_eq!(compiler::cpp_stdlib(target, None).unwrap(), "stdc++");
        assert_eq!(
            compiler::cpp_stdlib(target, Some("stdc++".into())).unwrap(),
            "stdc++"
        );
        let alternate = compiler::cpp_stdlib(target, Some("c++".into()));
        assert_eq!(alternate.is_ok(), target.contains("apple-darwin"));
        for invalid in ["", " ", "libc++", " c++", "stdc++ "] {
            assert!(compiler::cpp_stdlib(target, Some(invalid.into())).is_err());
        }
        assert!(!compiler::alternate_string_layout(target, "stdc++"));
        assert_eq!(
            compiler::alternate_string_layout(target, "c++"),
            target == "aarch64-apple-darwin"
        );
    }
}
