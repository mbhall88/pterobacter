//! Development binary seam: saved evidence and reference packages, through `terobacter-dev`.
//!
//! The binary exists only with the `dev` feature, so these tests do too.
#![cfg(feature = "dev")]

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use assert_cmd::cargo::cargo_bin_cmd;
use common::package::{
    sha256_hex, Synthetic, LICENCE_PATH, MANIFEST, NAMED_REGISTRY, PACKAGE_VERSION,
};
use serde_json::{json, Value};
use tempfile::TempDir;

const V1_SCHEMA: &str = "1.0.0-dev-portable-reference-package";
const PRIMARY_INDEX: &str = "databases/primary_whole/index.db";

fn package_on_disk(synthetic: &Synthetic) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    synthetic.write_to(dir.path());
    dir
}

// The operator vouches for the manifest digest the package has now, so a damaged package
// reaches its own rejection rather than the expected-digest check.
fn reference_package_run(directory: &Path, expected: &str) -> Output {
    cargo_bin_cmd!("terobacter-dev")
        .arg("reference-package")
        .arg(directory)
        .arg("--expected-sha256")
        .arg(expected)
        .output()
        .unwrap()
}

fn packaged_evidence_in(dir: &Path, package_sha256: &str) -> PathBuf {
    let path = dir.join("evidence.json");
    fs::write(&path, common::packaged_evidence(package_sha256).to_string()).unwrap();
    path
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

    let output = reference_package_run(dir.path(), &synthetic.manifest_sha256());

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
            |s: &mut Synthetic| s.remove(PRIMARY_INDEX),
            3,
            "missing",
        ),
        (
            "altered",
            |s: &mut Synthetic| s.alter(PRIMARY_INDEX),
            4,
            "altered",
        ),
        (
            "truncated",
            |s: &mut Synthetic| s.truncate(PRIMARY_INDEX),
            5,
            "truncated",
        ),
        (
            "frozen v1 schema",
            |s: &mut Synthetic| s.edit_manifest(|m| m["schema_version"] = json!(V1_SCHEMA)),
            6,
            "incompatible",
        ),
        (
            "blank licence disclaimer",
            |s: &mut Synthetic| {
                s.edit_json(LICENCE_PATH, |l| {
                    l["ncbi_submitter_rights_disclaimer"] = json!(" ")
                })
            },
            7,
            "inconsistent",
        ),
        (
            "unlisted",
            |s: &mut Synthetic| s.add_unlisted("databases/primary_whole/extra.tsv", b"extra"),
            10,
            "unlisted",
        ),
    ];
    for (case, damage, code, word) in cases {
        let mut synthetic = Synthetic::new();
        damage(&mut synthetic);
        let dir = package_on_disk(&synthetic);

        let output = reference_package_run(dir.path(), &synthetic.manifest_sha256());

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
fn a_manifest_that_is_not_the_expected_one_exits_with_its_own_code() {
    let expected = Synthetic::new().manifest_sha256();
    let mut replaced = Synthetic::new();
    replaced.edit_manifest(|m| m["package_id"] = json!("terobacter-reference-replacement"));
    let dir = package_on_disk(&replaced);

    let output = reference_package_run(dir.path(), &expected);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(9), "{stderr}");
    assert!(
        output.stdout.is_empty(),
        "a rejected package prints no identity"
    );
    assert!(
        stderr.starts_with("error:") && stderr.contains(&expected),
        "{stderr}"
    );
}

#[test]
fn registry_pins_must_match_the_listed_tables() {
    let synthetic = Synthetic::new();
    let dir = package_on_disk(&synthetic);
    let digest = synthetic.manifest_sha256();
    let listed = sha256_hex(synthetic.file(NAMED_REGISTRY));

    let matching = cargo_bin_cmd!("terobacter-dev")
        .arg("reference-package")
        .arg(dir.path())
        .arg("--expected-sha256")
        .arg(&digest)
        .arg("--named-registry-sha256")
        .arg(&listed)
        .output()
        .unwrap();
    assert!(
        matching.status.success(),
        "{}",
        String::from_utf8_lossy(&matching.stderr)
    );

    let mismatching = cargo_bin_cmd!("terobacter-dev")
        .arg("reference-package")
        .arg(dir.path())
        .arg("--expected-sha256")
        .arg(&digest)
        .arg("--named-registry-sha256")
        .arg("c".repeat(64))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&mismatching.stderr);
    assert_eq!(mismatching.status.code(), Some(9), "{stderr}");
    assert!(mismatching.stdout.is_empty());
    assert!(stderr.contains(NAMED_REGISTRY), "{stderr}");
}

#[test]
fn the_expected_digest_is_required_and_must_be_lowercase_hex() {
    let synthetic = Synthetic::new();
    let dir = package_on_disk(&synthetic);
    let path = dir.path().to_str().unwrap();
    let digest = synthetic.manifest_sha256();
    let upper = digest.to_uppercase();

    let cases: Vec<Vec<&str>> = vec![
        vec!["reference-package", path],
        vec!["reference-package", path, "--expected-sha256", "abc"],
        vec![
            "reference-package",
            path,
            "--expected-sha256",
            upper.as_str(),
        ],
        vec![
            "saved-evidence",
            common::FIXTURE_PATH,
            "--expected-sha256",
            digest.as_str(),
        ],
        vec![
            "saved-evidence",
            common::FIXTURE_PATH,
            "--reference-package",
            path,
        ],
        vec![
            "saved-evidence",
            common::FIXTURE_PATH,
            "--named-registry-sha256",
            digest.as_str(),
        ],
    ];
    for args in cases {
        let output = cargo_bin_cmd!("terobacter-dev")
            .args(&args)
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {stderr}");
    }
}

#[test]
fn a_directory_without_a_manifest_is_missing() {
    let dir = package_on_disk(&Synthetic::new());
    fs::remove_file(dir.path().join(MANIFEST)).unwrap();

    let output = reference_package_run(dir.path(), &Synthetic::new().manifest_sha256());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(3), "{stderr}");
    assert!(
        stderr.contains("missing") && stderr.contains(MANIFEST),
        "{stderr}"
    );
}

#[cfg(unix)]
#[test]
fn a_listed_file_that_is_a_symbolic_link_is_refused() {
    let synthetic = Synthetic::new();
    let dir = package_on_disk(&synthetic);
    let listed = "databases/primary_whole/index.db";
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("index.db");
    fs::write(&target, synthetic.files()[listed].as_slice()).unwrap();
    fs::remove_file(dir.path().join(listed)).unwrap();
    std::os::unix::fs::symlink(&target, dir.path().join(listed)).unwrap();

    let output = reference_package_run(dir.path(), &synthetic.manifest_sha256());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        output.stdout.is_empty(),
        "a refused read prints no identity"
    );
    assert!(stderr.contains("symbolic link"), "{stderr}");
}

#[test]
fn evidence_bound_to_the_verified_package_is_decided_and_reports_it() {
    let synthetic = Synthetic::new();
    let package_dir = package_on_disk(&synthetic);
    let evidence_dir = tempfile::tempdir().unwrap();
    let evidence_path = packaged_evidence_in(evidence_dir.path(), &synthetic.manifest_sha256());

    let output = cargo_bin_cmd!("terobacter-dev")
        .arg("saved-evidence")
        .arg(&evidence_path)
        .arg("--reference-package")
        .arg(package_dir.path())
        .arg("--expected-sha256")
        .arg(synthetic.manifest_sha256())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decision: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(decision["reference_package"]["status"], "verified");
    assert_eq!(
        decision["reference_package"]["package"]["version"],
        PACKAGE_VERSION
    );
}

#[test]
fn saved_evidence_bound_to_another_package_is_unresolved() {
    let synthetic = Synthetic::new();
    let package_dir = package_on_disk(&synthetic);
    let evidence_dir = tempfile::tempdir().unwrap();
    let evidence_path = packaged_evidence_in(evidence_dir.path(), &"c".repeat(64));

    let output = cargo_bin_cmd!("terobacter-dev")
        .arg("saved-evidence")
        .arg(&evidence_path)
        .arg("--reference-package")
        .arg(package_dir.path())
        .arg("--expected-sha256")
        .arg(synthetic.manifest_sha256())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(8), "{stderr}");
    assert!(
        output.stdout.is_empty(),
        "a binding failure prints no decision"
    );
    assert!(
        stderr.starts_with("error:") && stderr.contains("another reference package"),
        "{stderr}"
    );
}

#[test]
fn bound_problem_warnings_are_verified_and_changed_artifacts_produce_no_decision() {
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
