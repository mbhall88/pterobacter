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

#[test]
fn bound_problem_warnings_are_verified_and_changed_artifacts_produce_no_decision() {
    use serde_json::json;
    use sha2::{Digest, Sha256};
    let dir = tempfile::tempdir().unwrap();
    let evidence = dir.path().join("evidence.json");
    std::fs::write(&evidence, common::FIXTURE).unwrap();
    let digest = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    let raw: Value = serde_json::from_str(common::FIXTURE).unwrap();
    let artifact = dir.path().join("warning.txt");
    std::fs::write(&artifact, "supplied concern").unwrap();
    let envelope = json!({"schema_version":"1.0-shadow-problem-warnings",
        "query_sha256":raw["provenance"]["query_sha256"],
        "reference_package_sha256":raw["provenance"]["reference_package_sha256"],
        "evidence_sha256":digest(common::FIXTURE.as_bytes()),
        "warnings":[{"warning_id":"problem-1","reason":"Supplied concern","source":"synthetic-test","source_version":"1",
            "artifacts":[{"path":artifact,"sha256":digest(b"supplied concern") }]}]});
    let warnings = dir.path().join("warnings.json");
    let text = envelope.to_string();
    std::fs::write(&warnings, &text).unwrap();
    let mut command = cargo_bin_cmd!("terobacter-dev");
    command
        .arg("saved-evidence")
        .arg(&evidence)
        .arg("--problem-warnings")
        .arg(&warnings)
        .arg("--problem-warnings-sha256")
        .arg(digest(text.as_bytes()));
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let d: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(d["assignment"]["reason"], "external_problem_warning");
    assert_eq!(d["assignment"]["species_id"], Value::Null);
    std::fs::write(&artifact, "changed concern").unwrap();
    let output = command.output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("artifact SHA-256 mismatch"));
}
