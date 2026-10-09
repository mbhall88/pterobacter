//! Development binary seam: the same saved-evidence fixture, through `terobacter-dev`.
//!
//! The binary exists only with the `dev` feature, so these tests do too.
#![cfg(feature = "dev")]

mod common;

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;

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
}

#[test]
fn unreadable_or_invalid_evidence_is_an_error_with_no_decision() {
    let dir = tempfile::tempdir().unwrap();
    // Valid JSON of the right shape that fails the schema check, not just a parse error.
    let wrong_schema = dir.path().join("wrong-schema.json");
    std::fs::write(
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
