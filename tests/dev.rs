//! Development binary seam: saved evidence and reference packages, through `pterobacter-dev`.
//!
//! The binary exists only with the `dev` feature, so these tests do too.
#![cfg(feature = "dev")]

use common::hex;

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

#[test]
fn genomovar_diagnostic_is_exposed_only_by_the_development_binary() {
    let development_help = cargo_bin_cmd!("pterobacter-dev")
        .args(["genomovar-diagnostic", "--help"])
        .output()
        .unwrap();
    assert!(
        development_help.status.success(),
        "{}",
        String::from_utf8_lossy(&development_help.stderr)
    );
    let help = String::from_utf8_lossy(&development_help.stdout);
    for option in [
        "--report",
        "--expected-report-sha256",
        "--reference-package",
        "--mask-member",
        "--remove-group",
        "--exclude-evidence-row",
        "--setting",
        "--exclude-references",
        "--exclude-references-sha256",
        "--problem-warnings",
    ] {
        assert!(help.contains(option), "missing {option} from:\n{help}");
    }

    for binary in ["ptero", "pterobacter"] {
        let output = if binary == "ptero" {
            cargo_bin_cmd!("ptero").arg("--help").output().unwrap()
        } else {
            cargo_bin_cmd!("pterobacter")
                .arg("--help")
                .output()
                .unwrap()
        };
        assert!(output.status.success());
        let help = String::from_utf8_lossy(&output.stdout);
        assert!(!help.contains("genomovar-diagnostic"), "{binary}: {help}");
        assert!(!help.contains("--mask-member"), "{binary}: {help}");
        assert!(!help.contains("--remove-group"), "{binary}: {help}");
        assert!(!help.contains("--exclude-evidence-row"), "{binary}: {help}");
        let output = if binary == "ptero" {
            cargo_bin_cmd!("ptero")
                .args(["--exclude-evidence-row", "member"])
                .output()
                .unwrap()
        } else {
            cargo_bin_cmd!("pterobacter")
                .args(["--exclude-evidence-row", "member"])
                .output()
                .unwrap()
        };
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn genomovar_diagnostic_rejects_report_bytes_that_do_not_match_the_pin() {
    let report_dir = tempfile::tempdir().unwrap();
    let report = report_dir.path().join("report.json");
    fs::write(&report, "{}\n").unwrap();
    let output = cargo_bin_cmd!("pterobacter-dev")
        .args([
            "genomovar-diagnostic",
            "--report",
            report.to_str().unwrap(),
            "--expected-report-sha256",
            &"0".repeat(64),
            "--reference-package",
            "/not-read",
            "--expected-sha256",
            &"1".repeat(64),
            "--mask-member",
            "synthetic-member",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("report SHA-256 is"));
}

fn package_on_disk(synthetic: &Synthetic) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    synthetic.write_to(dir.path());
    dir
}

// The operator vouches for the manifest digest the package has now, so a damaged package
// reaches its own rejection rather than the expected-digest check.
fn reference_package_run(directory: &Path, expected: &str) -> Output {
    cargo_bin_cmd!("pterobacter-dev")
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
    let output = cargo_bin_cmd!("pterobacter-dev")
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
        let output = cargo_bin_cmd!("pterobacter-dev")
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

    let matching = cargo_bin_cmd!("pterobacter-dev")
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

    let mismatching = cargo_bin_cmd!("pterobacter-dev")
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
        let output = cargo_bin_cmd!("pterobacter-dev")
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

    let output = cargo_bin_cmd!("pterobacter-dev")
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

/// R2 and R4 take the whole primary panel from the authenticated package itself, so the
/// evaluation needs no separately produced inventory file.
#[test]
fn v2_decision_and_dry_plan_take_the_panel_from_a_verified_package() {
    let synthetic = Synthetic::new();
    let package_dir = package_on_disk(&synthetic);
    let evidence_dir = tempfile::tempdir().unwrap();
    let evidence_path = packaged_evidence_in(evidence_dir.path(), &synthetic.manifest_sha256());
    let mask = evidence_dir.path().join("mask.txt");
    let run = |command: &str, extra: &[&std::ffi::OsStr]| {
        let mut cmd = cargo_bin_cmd!("pterobacter-dev");
        cmd.arg(command).arg(&evidence_path);
        if command == "saved-evidence" {
            cmd.args(["--policy", "conservative-assembly-anim-v2"]);
        }
        cmd.arg("--reference-package")
            .arg(package_dir.path())
            .arg("--expected-sha256")
            .arg(synthetic.manifest_sha256())
            .args(extra)
            .output()
            .unwrap()
    };
    for command in ["saved-evidence", "rescue-plan"] {
        let output = run(command, &[]);
        assert!(
            output.status.success(),
            "{command}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            value["reference_package"]["status"], "verified",
            "{command}"
        );
        assert_eq!(
            value["reference_package"]["package"]["version"],
            PACKAGE_VERSION
        );
        assert_eq!(
            value["input"]["verification"],
            "input_and_artifact_hashes_checked; reference_package_verified"
        );
        assert_eq!(value["input"]["mask"]["panel_reference_count"], 5);

        let evidence = common::packaged_evidence(&synthetic.manifest_sha256());
        let reference = evidence["ranked_reference_evidence"][0]["reference_id"]
            .as_str()
            .unwrap()
            .to_owned();
        fs::write(&mask, format!("{reference}\n")).unwrap();
        let output = run(
            command,
            &["--exclude-references".as_ref(), mask.as_os_str()],
        );
        assert!(output.status.success(), "{command} masked");
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            value["input"]["mask"]["excluded_reference_ids"],
            serde_json::json!([reference])
        );

        // One panel source only.
        let inventory = evidence_dir.path().join("inventory.json");
        fs::write(&inventory, "{}").unwrap();
        let output = run(
            command,
            &["--reference-inventory".as_ref(), inventory.as_os_str()],
        );
        assert_ne!(output.status.code(), Some(0), "{command}");
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("either --reference-inventory or --reference-package"),
            "{command}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// P4b's Layer 2 v1 replay masks package-bound evidence. The package is checked against the
/// whole, unmasked evidence and supplies the panel; v1 is then applied to the masked evidence,
/// as v2 does behind `legacy_v1_result`.
#[test]
fn a_v1_mask_takes_the_panel_from_a_verified_package() {
    let synthetic = Synthetic::new();
    let package_dir = package_on_disk(&synthetic);
    let evidence_dir = tempfile::tempdir().unwrap();
    let evidence_path = packaged_evidence_in(evidence_dir.path(), &synthetic.manifest_sha256());
    let evidence = common::packaged_evidence(&synthetic.manifest_sha256());
    let reference = evidence["ranked_reference_evidence"][0]["reference_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mask = evidence_dir.path().join("mask.txt");
    fs::write(&mask, format!("{reference}\n")).unwrap();
    let run = |policy: &str| {
        let output = cargo_bin_cmd!("pterobacter-dev")
            .arg("saved-evidence")
            .arg(&evidence_path)
            .args(["--policy", policy])
            .arg("--reference-package")
            .arg(package_dir.path())
            .arg("--expected-sha256")
            .arg(synthetic.manifest_sha256())
            .arg("--exclude-references")
            .arg(&mask)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{policy}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    let v1 = run("conservative-assembly-v1");
    assert_eq!(v1["policy"]["id"], "conservative-assembly-v1");
    assert_eq!(v1["reference_package"]["status"], "verified");
    assert_eq!(
        v1["input"]["verification"],
        "input_and_artifact_hashes_checked; reference_package_verified"
    );
    assert_eq!(
        v1["input"]["mask"]["excluded_reference_ids"],
        serde_json::json!([reference])
    );
    assert_eq!(v1["input"]["mask"]["panel_reference_count"], 5);
    let v2 = run("conservative-assembly-anim-v2");
    let legacy = &v2["legacy_v1_result"];
    assert_eq!(legacy["status"], v1["assignment"]["status"]);
    assert_eq!(legacy["species_id"], v1["assignment"]["species_id"]);
    assert_eq!(legacy["subspecies_id"], v1["assignment"]["subspecies_id"]);
    assert_eq!(legacy["reasons"], v1["blocking_reasons"]);
}

/// A report directory inside the reference package would add unlisted files and invalidate
/// it. Every spelling of such a destination is refused before any directory is created, and
/// the package still verifies.
#[test]
fn a_report_directory_inside_the_reference_package_is_refused() {
    let synthetic = Synthetic::new();
    let package_dir = package_on_disk(&synthetic);
    let package = package_dir.path();
    let evidence_dir = tempfile::tempdir().unwrap();
    let evidence_path = packaged_evidence_in(evidence_dir.path(), &synthetic.manifest_sha256());
    let link = evidence_dir.path().join("package-link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(package, &link).unwrap();
    let verify = || {
        cargo_bin_cmd!("pterobacter-dev")
            .arg("reference-package")
            .arg(package)
            .arg("--expected-sha256")
            .arg(synthetic.manifest_sha256())
            .output()
            .unwrap()
    };
    let listing = || {
        let mut names: Vec<_> = fs::read_dir(package)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        names
    };
    assert!(verify().status.success());
    let before = listing();
    let mut destinations = vec![
        package.join("reports"),
        package.join("databases").join("..").join("reports"),
    ];
    if cfg!(unix) {
        destinations.push(link.join("reports"));
    }
    for policy in ["conservative-assembly-v1", "conservative-assembly-anim-v2"] {
        for destination in &destinations {
            let output = cargo_bin_cmd!("pterobacter-dev")
                .arg("saved-evidence")
                .arg(&evidence_path)
                .args(["--policy", policy])
                .arg("--reference-package")
                .arg(package)
                .arg("--expected-sha256")
                .arg(synthetic.manifest_sha256())
                .arg("--report-dir")
                .arg(destination)
                .output()
                .unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(
                output.status.code(),
                Some(2),
                "{policy} {destination:?}: {stderr}"
            );
            assert!(output.stdout.is_empty());
            assert!(stderr.contains("inside package"), "{stderr}");
            assert_eq!(listing(), before, "{policy} {destination:?}");
            assert!(verify().status.success(), "{policy} {destination:?}");
        }
    }

    // A destination outside the package still works.
    let reports = evidence_dir.path().join("reports");
    let output = cargo_bin_cmd!("pterobacter-dev")
        .arg("saved-evidence")
        .arg(&evidence_path)
        .arg("--reference-package")
        .arg(package)
        .arg("--expected-sha256")
        .arg(synthetic.manifest_sha256())
        .arg("--report-dir")
        .arg(&reports)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(reports.join("calls.tsv").is_file());
    assert_eq!(listing(), before);
    assert!(verify().status.success());
}

#[test]
fn saved_evidence_bound_to_another_package_is_unresolved() {
    let synthetic = Synthetic::new();
    let package_dir = package_on_disk(&synthetic);
    let evidence_dir = tempfile::tempdir().unwrap();
    let evidence_path = packaged_evidence_in(evidence_dir.path(), &"c".repeat(64));

    // v1, the v2 decision and the dry plan give the same exit code for the same binding error.
    for (command, policy) in [
        ("saved-evidence", "conservative-assembly-v1"),
        ("saved-evidence", "conservative-assembly-anim-v2"),
        ("rescue-plan", ""),
    ] {
        let mut cmd = cargo_bin_cmd!("pterobacter-dev");
        cmd.arg(command).arg(&evidence_path);
        if !policy.is_empty() {
            cmd.args(["--policy", policy]);
        }
        let output = cmd
            .arg("--reference-package")
            .arg(package_dir.path())
            .arg("--expected-sha256")
            .arg(synthetic.manifest_sha256())
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(8),
            "{command} {policy}: {stderr}"
        );
        assert!(
            output.stdout.is_empty(),
            "a binding failure prints no decision"
        );
        assert!(
            stderr.starts_with("error:") && stderr.contains("another reference package"),
            "{stderr}"
        );
    }
}

/// An edit to a JSON record, named by what it changes.
type Edit = fn(&mut Value);

#[test]
fn saved_evidence_contradicting_the_package_metadata_is_unresolved() {
    let synthetic = Synthetic::new();
    let package_dir = package_on_disk(&synthetic);
    let evidence_dir = tempfile::tempdir().unwrap();
    let edits: [(&str, Edit); 3] = [
        ("subsp. altera", |row| {
            row["subspecies_name"] = "Enterobacter syntheticus subsp. altera".into()
        }),
        ("name status", |row| {
            row["name_status"] = "validly_published_correct_name".into()
        }),
        ("role diversity", |row| {
            row["reference_role"] = "diversity".into()
        }),
    ];
    for (names, edit) in edits {
        let mut evidence = common::packaged_evidence(&synthetic.manifest_sha256());
        for row in evidence["ranked_reference_evidence"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .filter(|row| row["subspecies_id"] == "SYN-SS02")
        {
            edit(row);
        }
        let evidence_path = evidence_dir.path().join("evidence.json");
        fs::write(&evidence_path, evidence.to_string()).unwrap();

        let output = cargo_bin_cmd!("pterobacter-dev")
            .arg("saved-evidence")
            .arg(&evidence_path)
            .arg("--reference-package")
            .arg(package_dir.path())
            .arg("--expected-sha256")
            .arg(synthetic.manifest_sha256())
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(8), "{names}: {stderr}");
        assert!(output.stdout.is_empty(), "{names}: no decision is printed");
        assert!(stderr.contains(names), "{names}: {stderr}");
    }
}

#[test]
fn bound_problem_warnings_are_verified_and_changed_artifacts_produce_no_decision() {
    use sha2::{Digest, Sha256};
    let dir = tempfile::tempdir().unwrap();
    let evidence = dir.path().join("evidence.json");
    std::fs::write(&evidence, common::FIXTURE).unwrap();
    let digest = |bytes: &[u8]| hex::encode(&Sha256::digest(bytes));
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
    let mut command = cargo_bin_cmd!("pterobacter-dev");
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

mod v2 {
    use super::common::p1::{self, hit, reciprocal, unreturned, PROFILE};
    use assert_cmd::cargo::cargo_bin_cmd;
    use serde_json::{json, Value};
    use std::path::{Path, PathBuf};

    /// Species A nearest at 95.5 with a B contrast and an unreturned C reference.
    fn input() -> Value {
        json!({"references": [hit("A1", Some("A"), "95.5", "85", "85"),
                hit("B1", Some("B"), "90", "85", "85"), unreturned("C1", Some("C"))],
            "anim": {"A1": reciprocal("96.2", "85", "85"), "B1": reciprocal("90", "85", "85"),
                "C1": reciprocal("97", "85", "85")}})
    }

    struct Files {
        _dir: tempfile::TempDir,
        evidence: PathBuf,
        inventory: PathBuf,
        records: PathBuf,
        mask: PathBuf,
    }

    fn files(input: &Value) -> Files {
        let dir = tempfile::tempdir().unwrap();
        let write = |name: &str, text: &str| {
            let path = dir.path().join(name);
            std::fs::write(&path, text).unwrap();
            path
        };
        Files {
            evidence: write("evidence.json", &p1::evidence_text("dev", input)),
            inventory: write("inventory.json", &p1::inventory_text(input)),
            records: write("records.jsonl", &p1::records_text(input)),
            mask: write("mask.txt", "C1\n"),
            _dir: dir,
        }
    }

    fn run(args: &[&str], paths: &[&Path]) -> (Option<i32>, Value, String) {
        let mut command = cargo_bin_cmd!("pterobacter-dev");
        command.args(args);
        let output = command.args(paths).output().unwrap();
        let stdout = serde_json::from_slice(&output.stdout).unwrap_or(Value::Null);
        (
            output.status.code(),
            stdout,
            String::from_utf8_lossy(&output.stderr).into(),
        )
    }

    fn arg<'a>(f: &'a Files, name: &str) -> &'a Path {
        match name {
            "evidence" => &f.evidence,
            "inventory" => &f.inventory,
            "records" => &f.records,
            _ => &f.mask,
        }
    }

    /// Run `pterobacter-dev <command>` with options naming files from `f`.
    fn dev(command: &str, f: &Files, options: &[(&str, &str)]) -> (Option<i32>, Value, String) {
        let mut c = cargo_bin_cmd!("pterobacter-dev");
        c.arg(command).arg(&f.evidence);
        for (flag, value) in options {
            c.arg(flag);
            if ["evidence", "inventory", "records", "mask"].contains(value) {
                c.arg(arg(f, value));
            } else {
                c.arg(value);
            }
        }
        let output = c.output().unwrap();
        let stdout = serde_json::from_slice(&output.stdout).unwrap_or(Value::Null);
        (
            output.status.code(),
            stdout,
            String::from_utf8_lossy(&output.stderr).into(),
        )
    }

    #[test]
    fn saved_evidence_decides_v2_with_a_mask_and_plan_bound_anim() {
        let f = files(&input());
        let options = [
            ("--policy", "conservative-assembly-anim-v2"),
            ("--reference-inventory", "inventory"),
            ("--anim-records", "records"),
            ("--anim-method-profile", PROFILE),
        ];
        let (code, d, stderr) = dev("saved-evidence", &f, &options);
        assert_eq!(code, Some(0), "{stderr}");
        assert_eq!(d["reasons"], json!(["foreign_parent_challenge"]));

        let mut masked = options.to_vec();
        masked.push(("--exclude-references", "mask"));
        let (code, d, stderr) = dev("saved-evidence", &f, &masked);
        assert_eq!(code, Some(0), "{stderr}");
        assert_eq!(d["species_id"], "A");
        assert_eq!(d["call_basis"], "ANIm_assisted");
        assert_eq!(d["input"]["anim"]["binding"], "dry_plan_requested_pairs");
        assert_eq!(d["input"]["mask"]["excluded_reference_ids"], json!(["C1"]));
        assert_eq!(d["validation_status"], "not_yet_validated");
        assert_eq!(d["legacy_v1_result"]["status"], "unresolved");
    }

    #[test]
    fn v2_options_are_refused_where_they_do_not_apply() {
        let f = files(&input());
        let v1 = [
            ("--anim-records", "records"),
            ("--anim-method-profile", PROFILE),
            ("--reference-inventory", "inventory"),
        ];
        let (code, d, stderr) = dev("saved-evidence", &f, &v1);
        assert_eq!(code, Some(1));
        assert_eq!(d, Value::Null);
        assert!(stderr.contains("conservative-assembly-anim-v2"), "{stderr}");
        let no_profile = [
            ("--policy", "conservative-assembly-anim-v2"),
            ("--reference-inventory", "inventory"),
            ("--anim-records", "records"),
        ];
        let (code, _, stderr) = dev("saved-evidence", &f, &no_profile);
        assert_ne!(code, Some(0));
        assert!(stderr.contains("anim-method-profile"), "{stderr}");
        let (code, _, stderr) = dev(
            "saved-evidence",
            &f,
            &[("--policy", "conservative-assembly-anim-v2")],
        );
        assert_ne!(code, Some(0));
        assert!(stderr.contains("reference-inventory"), "{stderr}");
        assert!(stderr.contains("reference-package"), "{stderr}");
        // The panel comes from one source, so two are refused rather than reconciled.
        let bound = [
            ("--policy", "conservative-assembly-anim-v2"),
            ("--reference-inventory", "inventory"),
            ("--reference-package", crate::common::package::FIXTURE_DIR),
            (
                "--expected-sha256",
                crate::common::package::PINNED_MANIFEST_SHA256,
            ),
        ];
        let (code, d, stderr) = dev("saved-evidence", &f, &bound);
        assert_eq!(code, Some(1), "{stderr}");
        assert_eq!(d, Value::Null);
        assert!(
            stderr.contains("either --reference-inventory or --reference-package"),
            "{stderr}"
        );
    }

    /// The standalone v1 call under a reference mask, as P4b's Layer 2 replay needs it, is the
    /// same call v2 embeds as `legacy_v1_result` under that mask.
    #[test]
    fn v1_saved_evidence_applies_a_reference_mask_and_matches_the_v2_legacy_result() {
        let f = files(
            &json!({"references": [hit("A1", Some("A"), "97", "85", "85"),
            hit("B1", Some("B"), "90", "85", "85"), unreturned("C1", Some("C"))]}),
        );
        std::fs::write(&f.mask, "A1\n").unwrap();
        let (code, d, stderr) = dev("saved-evidence", &f, &[]);
        assert_eq!(code, Some(0), "{stderr}");
        assert_eq!(d["assignment"]["species_id"], "A");

        let masked = [
            ("--reference-inventory", "inventory"),
            ("--exclude-references", "mask"),
        ];
        let (code, v1, stderr) = dev("saved-evidence", &f, &masked);
        assert_eq!(code, Some(0), "{stderr}");
        assert_eq!(v1["policy"]["id"], "conservative-assembly-v1");
        assert_eq!(v1["assignment"]["status"], "unresolved");
        assert_eq!(v1["assignment"]["species_id"], Value::Null);
        // B1 is now nearest, and its only contrast, C1, was never measured.
        assert_eq!(
            v1["blocking_reasons"],
            json!(["species_competitor_unmeasured"])
        );
        assert_eq!(v1["input"]["mask"]["excluded_reference_ids"], json!(["A1"]));
        assert_eq!(v1["input"]["mask"]["panel_reference_count"], 3);

        let mut v2_options = masked.to_vec();
        v2_options.push(("--policy", "conservative-assembly-anim-v2"));
        let (code, v2, stderr) = dev("saved-evidence", &f, &v2_options);
        assert_eq!(code, Some(0), "{stderr}");
        let legacy = &v2["legacy_v1_result"];
        for (field, standalone) in [
            ("status", &v1["assignment"]["status"]),
            ("species_id", &v1["assignment"]["species_id"]),
            ("subspecies_id", &v1["assignment"]["subspecies_id"]),
            ("reasons", &v1["blocking_reasons"]),
        ] {
            assert_eq!(&legacy[field], standalone, "{field}");
        }
    }

    #[test]
    fn a_v1_mask_needs_the_primary_inventory_and_names_only_panel_references() {
        let f = files(&input());
        let (code, d, stderr) = dev("saved-evidence", &f, &[("--exclude-references", "mask")]);
        assert_eq!(code, Some(1));
        assert_eq!(d, Value::Null);
        assert!(stderr.contains("--reference-inventory"), "{stderr}");
        assert!(stderr.contains("--reference-package"), "{stderr}");

        std::fs::write(&f.mask, "Z9\n").unwrap();
        let (code, d, stderr) = dev(
            "saved-evidence",
            &f,
            &[
                ("--reference-inventory", "inventory"),
                ("--exclude-references", "mask"),
            ],
        );
        assert_eq!(code, Some(1));
        assert_eq!(d, Value::Null);
        assert!(stderr.contains("not in the primary panel"), "{stderr}");

        // The panel comes from one source.
        let (code, d, stderr) = dev(
            "saved-evidence",
            &f,
            &[
                ("--reference-inventory", "inventory"),
                ("--exclude-references", "mask"),
                ("--reference-package", crate::common::package::FIXTURE_DIR),
                (
                    "--expected-sha256",
                    crate::common::package::PINNED_MANIFEST_SHA256,
                ),
            ],
        );
        assert_eq!(code, Some(1));
        assert_eq!(d, Value::Null);
        assert!(
            stderr.contains("either --reference-inventory or --reference-package"),
            "{stderr}"
        );
    }

    /// P4b's RP-1 compares JSON, TSV and Markdown for every saved-evidence v2 record.
    #[test]
    fn saved_evidence_report_dir_writes_agreeing_json_tsv_and_markdown() {
        let f = files(&input());
        let dir = f._dir.path().join("reports");
        let dir_arg = dir.to_str().unwrap();
        let options = [
            ("--policy", "conservative-assembly-anim-v2"),
            ("--reference-inventory", "inventory"),
            ("--anim-records", "records"),
            ("--anim-method-profile", PROFILE),
            ("--exclude-references", "mask"),
            ("--report-dir", dir_arg),
        ];
        let (code, d, stderr) = dev("saved-evidence", &f, &options);
        assert_eq!(code, Some(0), "{stderr}");
        assert_eq!(d["species_id"], "A");
        assert_eq!(
            d["versions"]["policy"], "conservative-assembly-anim-v2",
            "the authoritative JSON carries the version block the renderings show"
        );
        let saved: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("report.json")).unwrap())
                .unwrap();
        assert_eq!(saved, d);
        let calls = std::fs::read_to_string(dir.join("calls.tsv")).unwrap();
        let mut lines = calls.lines();
        let header: Vec<_> = lines.next().unwrap().split('\t').collect();
        let row: Vec<_> = lines.next().unwrap().split('\t').collect();
        assert_eq!(lines.next(), None);
        let cell = |name: &str| row[header.iter().position(|h| *h == name).unwrap()];
        assert_eq!(cell("call_status"), "assigned");
        assert_eq!(cell("species_id"), "A");
        assert_eq!(cell("call_basis"), "ANIm_assisted");
        assert_eq!(cell("policy_id"), "conservative-assembly-anim-v2");
        assert_eq!(cell("validation_status"), "not_yet_validated");
        assert!(std::fs::read_to_string(dir.join("evidence.tsv"))
            .unwrap()
            .contains("A1"));
        assert!(std::fs::read_to_string(dir.join("report.md"))
            .unwrap()
            .contains("Synthetic A"));

        // An existing directory is refused before any work, and nothing in it is replaced.
        let before = std::fs::read(dir.join("report.json")).unwrap();
        let (code, d, stderr) = dev("saved-evidence", &f, &options);
        assert_eq!(code, Some(2), "{stderr}");
        assert_eq!(d, Value::Null);
        assert!(stderr.contains("--report-dir"), "{stderr}");
        assert_eq!(std::fs::read(dir.join("report.json")).unwrap(), before);

        // A failed run removes the directory it made, so the same command can be rerun.
        let failed_dir = f._dir.path().join("failed-reports");
        let failing = [
            ("--policy", "conservative-assembly-anim-v2"),
            ("--report-dir", failed_dir.to_str().unwrap()),
        ];
        let (code, d, stderr) = dev("saved-evidence", &f, &failing);
        assert_eq!(code, Some(1), "{stderr}");
        assert_eq!(d, Value::Null);
        assert!(!failed_dir.exists(), "{stderr}");

        let v1_dir = f._dir.path().join("v1-reports");
        let (code, _, stderr) = dev(
            "saved-evidence",
            &f,
            &[("--report-dir", v1_dir.to_str().unwrap())],
        );
        assert_eq!(code, Some(0), "{stderr}");
        let calls = std::fs::read_to_string(v1_dir.join("calls.tsv")).unwrap();
        assert!(calls.contains("conservative-assembly-v1"), "{calls}");
    }

    #[test]
    fn rescue_plan_reports_witnesses_then_outstanding_competitors() {
        let mut first = input();
        first["anim"] = json!({});
        let f = files(&first);
        let (code, p, stderr) = dev("rescue-plan", &f, &[("--reference-inventory", "inventory")]);
        assert_eq!(code, Some(0), "{stderr}");
        assert_eq!(p["trigger_state"], "triggered_with_witness_candidate");
        assert_eq!(p["nearest_parent"]["concept_id"], "A");
        assert_eq!(p["outstanding_witnesses"], json!(["A1"]));

        let mut witness = input();
        witness["anim"] = json!({"A1": reciprocal("96.2", "85", "85")});
        let f = files(&witness);
        let (code, p, stderr) = dev(
            "rescue-plan",
            &f,
            &[
                ("--reference-inventory", "inventory"),
                ("--anim-records", "records"),
                ("--anim-method-profile", PROFILE),
            ],
        );
        assert_eq!(code, Some(0), "{stderr}");
        assert_eq!(p["stage"], "competitor_panel");
        assert_eq!(p["outstanding_competitors"], json!(["B1", "C1"]));
    }

    #[test]
    fn rescue_plan_runs_no_alignment() {
        // The plan takes no sequence input, needs no executable on PATH, and leaves its
        // working directory untouched.
        let f = files(&input());
        let work = tempfile::tempdir().unwrap();
        let output = cargo_bin_cmd!("pterobacter-dev")
            .env_clear()
            .current_dir(work.path())
            .arg("rescue-plan")
            .arg(&f.evidence)
            .arg("--reference-inventory")
            .arg(&f.inventory)
            .arg("--anim-records")
            .arg(&f.records)
            .args(["--anim-method-profile", PROFILE])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let p: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(p["anim_executed"], false);
        assert_eq!(p["counts"]["completed"], 3);
        assert_eq!(std::fs::read_dir(work.path()).unwrap().count(), 0);

        let (code, _, stderr) = run(&["rescue-plan", "--query"], &[&f.evidence, &f.evidence]);
        assert_eq!(code, Some(2));
        assert!(stderr.contains("unexpected argument"), "{stderr}");
    }

    #[test]
    fn a_records_directory_is_read_whole_and_a_changed_input_hash_is_refused() {
        let f = files(&input());
        let dir = tempfile::tempdir().unwrap();
        let text = std::fs::read_to_string(&f.records).unwrap();
        let mut lines = text.lines();
        std::fs::write(dir.path().join("wave-0001.jsonl"), lines.next().unwrap()).unwrap();
        std::fs::write(
            dir.path().join("wave-0002.jsonl"),
            lines.collect::<Vec<_>>().join("\n"),
        )
        .unwrap();
        let (code, d, stderr) = run(
            &[
                "saved-evidence",
                "--policy",
                "conservative-assembly-anim-v2",
                "--anim-method-profile",
                PROFILE,
                "--reference-inventory-sha256",
                &"0".repeat(64),
            ],
            &[
                &f.evidence,
                Path::new("--reference-inventory"),
                &f.inventory,
                Path::new("--anim-records"),
                dir.path(),
            ],
        );
        assert_eq!(code, Some(1));
        assert_eq!(d, Value::Null);
        assert!(stderr.contains("SHA-256 mismatch"), "{stderr}");
        let (code, d, stderr) = run(
            &[
                "saved-evidence",
                "--policy",
                "conservative-assembly-anim-v2",
                "--anim-method-profile",
                PROFILE,
            ],
            &[
                &f.evidence,
                Path::new("--reference-inventory"),
                &f.inventory,
                Path::new("--anim-records"),
                dir.path(),
            ],
        );
        assert_eq!(code, Some(0), "{stderr}");
        assert_eq!(d["counts"]["completed"], 3);
        assert_eq!(d["input"]["anim_record_files"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn records_without_a_query_id_produce_no_decision() {
        let f = files(&input());
        // Swapped native IDs that only a query ID could expose.
        let text = std::fs::read_to_string(&f.records)
            .unwrap()
            .replace("\"id\":\"query\",", "")
            .replace(
                "\"native_query_id\":\"query\"",
                "\"native_query_id\":\"swapped\"",
            );
        std::fs::write(&f.records, text).unwrap();
        let (code, d, stderr) = dev(
            "saved-evidence",
            &f,
            &[
                ("--policy", "conservative-assembly-anim-v2"),
                ("--reference-inventory", "inventory"),
                ("--anim-records", "records"),
                ("--anim-method-profile", PROFILE),
                ("--exclude-references", "mask"),
            ],
        );
        assert_eq!(code, Some(1));
        assert_eq!(d, Value::Null);
        assert!(stderr.contains("ANIm query ID required"), "{stderr}");
    }

    #[test]
    fn a_pretty_printed_json_record_file_is_read_as_one_record() {
        let mut witness = input();
        witness["anim"] = json!({"A1": reciprocal("96.2", "85", "85")});
        let f = files(&witness);
        let line = std::fs::read_to_string(&f.records).unwrap();
        let record: Value = serde_json::from_str(line.trim()).unwrap();
        let pretty = f.records.with_file_name("A1.json");
        std::fs::write(&pretty, serde_json::to_string_pretty(&record).unwrap()).unwrap();
        let (code, p, stderr) = run(
            &["rescue-plan", "--anim-method-profile", PROFILE],
            &[
                &f.evidence,
                Path::new("--reference-inventory"),
                &f.inventory,
                Path::new("--anim-records"),
                &pretty,
            ],
        );
        assert_eq!(code, Some(0), "{stderr}");
        assert_eq!(p["passing_witness"], "A1");
        assert_eq!(p["input"]["anim_record_files"][0]["records"], 1);
    }
}
