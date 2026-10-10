//! Custom threshold flags through the agreed saved-evidence and dry-plan seams.
#![cfg(feature = "dev")]
mod common;

use assert_cmd::cargo::cargo_bin_cmd;
use common::p1::{self, hit};
use serde_json::{json, Value};

fn run(operation: &str, input: &Value, options: &[&str]) -> std::process::Output {
    let dir = tempfile::tempdir().unwrap();
    let evidence = dir.path().join("evidence.json");
    let inventory = dir.path().join("inventory.json");
    std::fs::write(&evidence, p1::evidence_text("custom", input)).unwrap();
    std::fs::write(&inventory, p1::inventory_text(input)).unwrap();
    let mut cmd = cargo_bin_cmd!("terobacter-dev");
    cmd.arg(operation)
        .arg(evidence)
        .arg("--reference-inventory")
        .arg(inventory);
    if operation == "saved-evidence" {
        cmd.args(["--policy", "conservative-assembly-anim-v2"]);
    }
    cmd.args(options).output().unwrap()
}

fn decision(output: std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn input(ani: &str) -> Value {
    json!({"references": [hit("A1", Some("A"), ani, "85", "85"),
        hit("B1", Some("B"), "80", "85", "85")]})
}

#[test]
fn explicitly_supplied_defaults_are_custom_in_both_operations() {
    for operation in ["saved-evidence", "rescue-plan"] {
        for flags in [
            vec!["--species-ani", "96"],
            vec!["--subspecies-ani", "98"],
            vec!["--species-ani", "96", "--subspecies-ani", "98"],
        ] {
            let d = decision(run(operation, &input("95"), &flags));
            assert_eq!(d["policy_mode"], "custom");
            assert_eq!(d["validation_status"], "custom_unvalidated");
            assert_eq!(d["thresholds"]["species_ani"], "96");
            assert_eq!(d["thresholds"]["subspecies_ani"], "98");
        }
    }
}

#[test]
fn invalid_thresholds_fail_before_reading_inputs_or_printing_a_result() {
    for flags in [
        vec!["--species-ani", "NaN"],
        vec!["--species-ani", "inf"],
        vec!["--species-ani", "96e0"],
        vec!["--species-ani", "79.999999999999"],
        vec!["--species-ani", "98.01"],
        vec!["--subspecies-ani", "95.99"],
        vec!["--subspecies-ani", "100.000000000001"],
        vec!["--species-ani", "96", "--subspecies-ani", "95"],
    ] {
        for operation in ["saved-evidence", "rescue-plan"] {
            let mut cmd = cargo_bin_cmd!("terobacter-dev");
            cmd.args([operation, "missing.json"]);
            if operation == "saved-evidence" {
                cmd.args(["--policy", "conservative-assembly-anim-v2"]);
            }
            let out = cmd.args(&flags).output().unwrap();
            assert!(!out.status.success(), "{flags:?}");
            assert!(out.stdout.is_empty());
            let error = String::from_utf8_lossy(&out.stderr);
            assert!(
                error.contains("decimal") || error.contains("80 <="),
                "{error}"
            );
        }
    }
}

#[test]
fn v1_refuses_each_override_even_when_equal_to_the_default() {
    for flag in ["--species-ani=96", "--subspecies-ani=98"] {
        let out = cargo_bin_cmd!("terobacter-dev")
            .args(["saved-evidence", "missing.json", flag])
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(out.stdout.is_empty());
        assert!(String::from_utf8_lossy(&out.stderr).contains("v1 rejects threshold overrides"));
    }
}

#[test]
fn equivalent_decimal_spellings_share_a_digest_but_preserve_supplied_text() {
    for operation in ["saved-evidence", "rescue-plan"] {
        let a = decision(run(operation, &input("95"), &["--species-ani", "96"]));
        let b = decision(run(
            operation,
            &input("95"),
            &["--species-ani", "96.000", "--subspecies-ani", "98.0"],
        ));
        let changed = decision(run(operation, &input("95"), &["--species-ani", "96.01"]));
        let default = decision(run(operation, &input("95"), &[]));
        let digest = a["settings_sha256"].as_str().expect("settings digest");
        assert_eq!(digest.len(), 64);
        assert_eq!(a["settings_sha256"], b["settings_sha256"]);
        assert_ne!(a["settings_sha256"], changed["settings_sha256"]);
        assert_ne!(a["settings_sha256"], default["settings_sha256"]);
        assert_eq!(a["thresholds"]["supplied"]["species_ani"], "96");
        assert_eq!(b["thresholds"]["supplied"]["species_ani"], "96.000");
        assert_eq!(b["thresholds"]["supplied"]["subspecies_ani"], "98.0");
    }
}

#[test]
fn cli_overrides_change_the_trigger_and_the_ordinary_species_gate() {
    let p = decision(run(
        "rescue-plan",
        &input("95.5"),
        &["--species-ani", "95.505"],
    ));
    assert_eq!(p["trigger"], true);
    assert_eq!(p["thresholds"]["trigger_lower_ani"], "94.005");
    let d = decision(run(
        "saved-evidence",
        &input("95.5"),
        &["--species-ani", "95.5"],
    ));
    assert_eq!(d["trigger"], false);
    assert_eq!(d["species_id"], "A");
    assert_eq!(d["call_basis"], "skani");
    assert_eq!(d["legacy_v1_result"]["species_id"], Value::Null);
}

#[test]
fn arbitrary_decimal_precision_is_accepted_without_rounding_or_losing_literals() {
    let zeros = decision(run(
        "rescue-plan",
        &input("95.5"),
        &["--species-ani", "96.0000000000000000000000"],
    ));
    let ordinary = decision(run("rescue-plan", &input("95.5"), &["--species-ani", "96"]));
    assert_eq!(zeros["settings_sha256"], ordinary["settings_sha256"]);
    assert_eq!(
        zeros["thresholds"]["supplied"]["species_ani"],
        "96.0000000000000000000000"
    );
    let p = decision(run(
        "rescue-plan",
        &input("95.5"),
        &["--species-ani", "95.5000000000000000000001"],
    ));
    assert_eq!(p["trigger"], true);
    assert_eq!(
        p["thresholds"]["trigger_lower_ani"],
        "94.0000000000000000000001"
    );
    assert_eq!(p["thresholds"]["species_ani"], "95.5000000000000000000001");
}
