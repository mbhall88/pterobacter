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
        let mut command = cargo_bin_cmd!("terobacter-dev");
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

    /// Run `terobacter-dev <command>` with options naming files from `f`.
    fn dev(command: &str, f: &Files, options: &[(&str, &str)]) -> (Option<i32>, Value, String) {
        let mut c = cargo_bin_cmd!("terobacter-dev");
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
        let output = cargo_bin_cmd!("terobacter-dev")
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
