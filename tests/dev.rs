//! Development binary seam: the same saved-evidence fixture, through `terobacter-dev`.
//!
//! The binary exists only with the `dev` feature, so these tests do too.
#![cfg(feature = "dev")]

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/v1-species-assigned/evidence.json"
);
const FROZEN_CALL: &str = include_str!("fixtures/v1-species-assigned/frozen-v1-call.json");

#[test]
fn saved_evidence_fixture_reproduces_its_frozen_v1_species_call_as_json() {
    let frozen: Value = serde_json::from_str(FROZEN_CALL).unwrap();

    let output = cargo_bin_cmd!("terobacter-dev")
        .args(["saved-evidence", FIXTURE])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decision: Value =
        serde_json::from_slice(&output.stdout).expect("stdout is the JSON decision");
    assert_eq!(decision["policy"]["id"], frozen["policy"]);
    assert_eq!(decision["species"]["status"], frozen["status"]);
    assert_eq!(decision["species"]["id"], frozen["species_id"]);
    assert_eq!(decision["species"]["name"], frozen["species_name"]);
}

#[test]
fn unreadable_or_invalid_evidence_is_an_error_with_no_decision() {
    let dir = tempfile::tempdir().unwrap();
    let not_evidence = dir.path().join("not-evidence.json");
    std::fs::write(&not_evidence, "{\"schema_version\": \"1.0.0\"}").unwrap();
    let missing = dir.path().join("missing.json");

    for path in [&not_evidence, &missing] {
        let output = cargo_bin_cmd!("terobacter-dev")
            .arg("saved-evidence")
            .arg(path)
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(1), "{path:?}");
        assert!(
            output.stdout.is_empty(),
            "a failed run must not print a decision: {path:?}"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).starts_with("error:"),
            "{path:?}"
        );
    }
}
