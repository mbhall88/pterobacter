//! Development binary seam: saved evidence and reference packages, through `terobacter-dev`.
//!
//! The binary exists only with the `dev` feature, so these tests do too.
#![cfg(feature = "dev")]

mod common;

use std::fs;

use assert_cmd::cargo::cargo_bin_cmd;
use common::package::{Synthetic, LICENCE_PATH, MANIFEST, PACKAGE_VERSION};
use serde_json::{json, Value};
use tempfile::TempDir;

fn package_on_disk(synthetic: &Synthetic) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    synthetic.write_to(dir.path());
    dir
}

#[test]
fn saved_evidence_fixture_reproduces_its_frozen_v1_species_call_as_json() {
    let output = cargo_bin_cmd!("terobacter-dev")
        .args(["saved-evidence", common::FIXTURE_PATH])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decision: Value =
        serde_json::from_slice(&output.stdout).expect("stdout is the JSON decision");
    common::assert_matches_frozen_call(&decision);
    assert_eq!(decision["reference_package"]["status"], "not_verified");
}

#[test]
fn unreadable_or_invalid_evidence_is_an_error_with_no_decision() {
    let dir = tempfile::tempdir().unwrap();
    // Valid JSON of the right shape that fails the schema check, not just a parse error.
    let wrong_schema = dir.path().join("wrong-schema.json");
    fs::write(
        &wrong_schema,
        common::FIXTURE.replace("2.0.0-dev-evidence", "1.0.0"),
    )
    .unwrap();
    let missing = dir.path().join("missing.json");

    for (path, expected) in [
        (&wrong_schema, "expected schema"),
        (&missing, "No such file"),
    ] {
        let output = cargo_bin_cmd!("terobacter-dev")
            .arg("saved-evidence")
            .arg(path)
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{path:?}");
        assert!(
            output.stdout.is_empty(),
            "a failed run must not print a decision: {path:?}"
        );
        assert!(
            stderr.starts_with("error:") && stderr.contains(expected),
            "{path:?}: {stderr}"
        );
    }
}

#[test]
fn a_verified_reference_package_prints_its_identity_and_components() {
    let synthetic = Synthetic::new();
    let dir = package_on_disk(&synthetic);

    let output = cargo_bin_cmd!("terobacter-dev")
        .arg("reference-package")
        .arg(dir.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).expect("stdout is JSON");
    assert_eq!(report["package"]["version"], PACKAGE_VERSION);
    assert_eq!(
        report["package"]["manifest_sha256"],
        synthetic.manifest_sha256()
    );
    let sketches = &report["components"]["genomovar_chromosome"]["databases"]["sketches.db"];
    assert_eq!(sketches["sha256"].as_str().map(str::len), Some(64));
}

#[test]
fn each_rejected_reference_package_exits_with_its_own_code_and_names_the_fault() {
    type Damage = fn(&mut Synthetic);
    let cases: Vec<(&str, Damage, i32, &str)> = vec![
        (
            "missing",
            |s: &mut Synthetic| s.remove("databases/primary_whole/index.db"),
            3,
            "missing",
        ),
        (
            "altered",
            |s: &mut Synthetic| s.alter("databases/primary_whole/index.db"),
            4,
            "altered",
        ),
        (
            "truncated",
            |s: &mut Synthetic| s.truncate("databases/primary_whole/index.db"),
            5,
            "truncated",
        ),
        (
            "incompatible",
            |s: &mut Synthetic| s.set_manifest("schema_version", json!("0.9.0")),
            6,
            "incompatible",
        ),
        (
            "incomplete",
            |s: &mut Synthetic| s.drop_listing(LICENCE_PATH),
            7,
            "incomplete",
        ),
    ];
    for (case, damage, code, word) in cases {
        let mut synthetic = Synthetic::new();
        damage(&mut synthetic);
        let dir = package_on_disk(&synthetic);

        let output = cargo_bin_cmd!("terobacter-dev")
            .arg("reference-package")
            .arg(dir.path())
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(code), "{case}: {stderr}");
        assert!(
            output.stdout.is_empty(),
            "{case}: a rejected package prints no identity"
        );
        assert!(
            stderr.starts_with("error:") && stderr.contains(word),
            "{case}: {stderr}"
        );
    }
}

#[test]
fn a_directory_without_a_manifest_is_missing() {
    let dir = package_on_disk(&Synthetic::new());
    fs::remove_file(dir.path().join(MANIFEST)).unwrap();

    let output = cargo_bin_cmd!("terobacter-dev")
        .arg("reference-package")
        .arg(dir.path())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(3), "{stderr}");
    assert!(
        stderr.contains("missing") && stderr.contains(MANIFEST),
        "{stderr}"
    );
}

#[test]
fn evidence_bound_to_the_verified_package_is_decided_and_reports_it() {
    let synthetic = Synthetic::new();
    let package_dir = package_on_disk(&synthetic);
    let mut evidence: Value = serde_json::from_str(common::FIXTURE).unwrap();
    evidence["provenance"]["reference_package_sha256"] = synthetic.manifest_sha256().into();
    let evidence_dir = tempfile::tempdir().unwrap();
    let evidence_path = evidence_dir.path().join("evidence.json");
    fs::write(&evidence_path, evidence.to_string()).unwrap();

    let output = cargo_bin_cmd!("terobacter-dev")
        .arg("saved-evidence")
        .arg(&evidence_path)
        .arg("--reference-package")
        .arg(package_dir.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decision: Value = serde_json::from_slice(&output.stdout).unwrap();
    common::assert_matches_frozen_call(&decision);
    assert_eq!(decision["reference_package"]["status"], "verified");
}

#[test]
fn saved_evidence_bound_to_another_package_is_not_bound() {
    let package_dir = package_on_disk(&Synthetic::new());

    let output = cargo_bin_cmd!("terobacter-dev")
        .arg("saved-evidence")
        .arg(common::FIXTURE_PATH)
        .arg("--reference-package")
        .arg(package_dir.path())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(8), "{stderr}");
    assert!(
        output.stdout.is_empty(),
        "a binding failure prints no decision"
    );
    assert!(
        stderr.starts_with("error:") && stderr.contains("not bound"),
        "{stderr}"
    );
}
