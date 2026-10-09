//! Packaging seam: what a default build and `cargo install` expose.
//!
//! `cargo install` builds every binary whose required features are enabled, so a default
//! install exposes exactly the targets with no required features.

use std::process::Command;

use serde_json::Value;

fn package() -> Value {
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--offline",
        ])
        .arg("--manifest-path")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .output()
        .expect("cargo metadata should run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        metadata["packages"].as_array().unwrap().len(),
        1,
        "one crate only"
    );
    metadata["packages"][0].clone()
}

fn binary_targets() -> Vec<(String, Vec<String>)> {
    package()["targets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["kind"][0] == "bin")
        .map(|t| {
            let features = t["required-features"]
                .as_array()
                .map_or_else(Vec::new, |f| {
                    f.iter().map(|x| x.as_str().unwrap().to_owned()).collect()
                });
            (t["name"].as_str().unwrap().to_owned(), features)
        })
        .collect()
}

#[test]
fn a_default_install_exposes_only_tero_and_terobacter() {
    let mut installed: Vec<String> = binary_targets()
        .into_iter()
        .filter(|(_, required)| required.is_empty())
        .map(|(name, _)| name)
        .collect();
    installed.sort();

    assert_eq!(installed, ["tero", "terobacter"]);
}

#[test]
fn the_development_feature_is_not_enabled_by_default() {
    let default_features = package()["features"]["default"].clone();

    assert!(
        default_features.is_null() || !default_features.to_string().contains("dev"),
        "dev must stay out of default builds and installs: {default_features}"
    );
}

#[test]
fn the_development_binary_is_built_only_with_its_feature() {
    let dev: Vec<_> = binary_targets()
        .into_iter()
        .filter(|(name, _)| name == "terobacter-dev")
        .collect();

    assert_eq!(dev, [("terobacter-dev".to_owned(), vec!["dev".to_owned()])]);
}
