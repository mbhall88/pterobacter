//! Native R1 v2 workflow, using frozen synthetic derivations only.
use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
mod common;
use common::assembly::native_package;

#[test]
fn outside_band_v2_uses_the_same_skani_call_without_requiring_fastas() {
    let (dir, pin, seq) = native_package();
    let query = dir.path().join("query.fa");
    std::fs::write(&query, [b">synthetic\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    let output = cargo_bin_cmd!("tero")
        .arg("--reference-package")
        .arg(dir.path().join("package"))
        .args([
            "--expected-sha256",
            &pin,
            "--policy",
            "conservative-assembly-anim-v2",
        ])
        .arg(query)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["policy_id"], "conservative-assembly-anim-v2");
    assert_eq!(report["species_id"], "SYN-SP01");
    assert_eq!(report["subspecies_id"], "SYN-SS01");
    assert_eq!(report["legacy_v1_result"]["species_id"], "SYN-SP01");
    assert_eq!(report["rescue_status"], "not_triggered");
    assert_eq!(report["counts"]["attempted"], 0);
    assert_eq!(report["execution"]["status"], "complete");
    assert_eq!(report["validation_status"], "not_yet_validated");
}

#[test]
fn triggered_query_missing_fastas_preserves_the_v2_plan_and_exits_incomplete() {
    let (dir, pin, seq) = common::assembly::rescue_package();
    let query = dir.path().join("query.fa");
    std::fs::write(
        &query,
        [b">synthetic\n".as_slice(), &seq, b"\n>short\nACGTN\n"].concat(),
    )
    .unwrap();
    let output = cargo_bin_cmd!("tero")
        .arg("--reference-package")
        .arg(dir.path().join("package"))
        .args([
            "--expected-sha256",
            &pin,
            "--policy",
            "conservative-assembly-anim-v2",
        ])
        .arg(query)
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!output.status.success(), "{report}");
    assert_eq!(report["trigger"], true, "{report}");
    assert_eq!(report["anim_plan"]["stage"], "witness", "{report}");
    assert_eq!(report["execution"]["status"], "incomplete");
    assert_eq!(report["species_id"], Value::Null);
    assert_eq!(report["counts"]["attempted"], 0);
    assert_eq!(report["input"]["total_bases"], 200008);
    assert_eq!(report["anim_execution"]["stop_reason"], "missing_component");
}

#[test]
fn native_rescue_measures_every_primary_competitor_including_unreturned() {
    let (dir, pin, seq) = common::assembly::rescue_package();
    let query = dir.path().join("query.fa");
    std::fs::write(
        &query,
        [b">synthetic\n".as_slice(), &seq, b"\n>short\nACGTN\n"].concat(),
    )
    .unwrap();
    let anim_pin =
        common::package::sha256_hex(&std::fs::read(dir.path().join("fastas/anim.json")).unwrap());
    let output = cargo_bin_cmd!("tero")
        .arg("--reference-package")
        .arg(dir.path().join("package"))
        .args([
            "--expected-sha256",
            &pin,
            "--policy",
            "conservative-assembly-anim-v2",
            "--expected-anim-sha256",
            &anim_pin,
        ])
        .arg("--anim-reference-package")
        .arg(dir.path().join("fastas"))
        .arg("--anim-output")
        .arg(dir.path().join("receipts"))
        .arg(&query)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let mut report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["rescue_status"], "rescued", "{report}");
    assert_eq!(report["species_id"], "SYN-SP01");
    assert_eq!(report["subspecies_id"], Value::Null);
    assert_eq!(report["genomovar_id"], Value::Null);
    assert_eq!(report["call_basis"], "ANIm_assisted");
    assert_eq!(report["competitor_evaluation"], "complete");
    assert_eq!(report["counts"]["required"], 5);
    assert_eq!(report["counts"]["attempted"], 5);
    assert_eq!(report["counts"]["completed"], 5);
    assert_eq!(report["counts"]["no_alignment"], 1);
    assert_eq!(report["counts"]["not_run"], 0);
    assert_eq!(
        report["measurement"]["not_returned_reference_files"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let pairs = report["anim_execution"]["pairs"].as_array().unwrap();
    assert_eq!(pairs.len(), 5);
    for pair in pairs {
        let root = std::path::Path::new(pair["directory"].as_str().unwrap());
        assert!(root.join("result.json").exists());
        assert_eq!(
            pair["record"]["query"]["sha256"],
            report["input"]["query_sha256"]
        );
        assert_eq!(pair["record"]["reference"]["component"], "primary_whole");
    }
    let receipts = report["anim_execution"]["pairs"].clone();
    assert!(terobacter::cli::v2::cancel_before_publication(
        std::slice::from_mut(&mut report),
        &std::sync::atomic::AtomicBool::new(true)
    ));
    assert_eq!(report["execution"]["stage"], "publication");
    assert_eq!(report["execution"]["status"], "incomplete");
    assert_eq!(report["species_id"], Value::Null);
    assert_eq!(report["rescue_status"], "incomplete");
    assert_eq!(report["anim_execution"]["pairs"], receipts);
}

struct Prepared {
    dir: tempfile::TempDir,
    pin: String,
    anim_pin: String,
    query: std::path::PathBuf,
}
impl Prepared {
    fn new(case: common::assembly::NativeCase) -> Self {
        let (dir, pin, seq) = common::assembly::native_case(case);
        let query = dir.path().join("query.fa");
        std::fs::write(&query, [b">synthetic\n".as_slice(), &seq, b"\n"].concat()).unwrap();
        let anim_pin = common::package::sha256_hex(
            &std::fs::read(dir.path().join("fastas/anim.json")).unwrap(),
        );
        Self {
            dir,
            pin,
            anim_pin,
            query,
        }
    }
    fn command(&self) -> assert_cmd::Command {
        let mut command = cargo_bin_cmd!("tero");
        command
            .arg("--reference-package")
            .arg(self.dir.path().join("package"))
            .args([
                "--expected-sha256",
                &self.pin,
                "--policy",
                "conservative-assembly-anim-v2",
                "--expected-anim-sha256",
                &self.anim_pin,
            ])
            .arg("--anim-reference-package")
            .arg(self.dir.path().join("fastas"))
            .arg("--anim-output")
            .arg(self.dir.path().join("receipts"))
            .arg(&self.query);
        command
    }
    fn edit_manifest(&mut self, edit: impl FnOnce(&mut Value)) {
        let path = self.dir.path().join("fastas/anim.json");
        let mut value: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        edit(&mut value);
        let bytes = serde_json::to_vec_pretty(&value).unwrap();
        self.anim_pin = common::package::sha256_hex(&bytes);
        std::fs::write(path, bytes).unwrap();
    }
}

#[test]
fn component_is_bound_to_full_panel_scope_concepts_and_exact_sources() {
    use common::assembly::NativeCase::Rescue;
    let edits: Vec<fn(&mut Value)> = vec![
        |v| v["reference_package_sha256"] = "a".repeat(64).into(),
        |v| v["sequence_scope"] = "chromosome".into(),
        |v| {
            v["references"].as_array_mut().unwrap().pop();
        },
        |v| v["references"][1] = v["references"][0].clone(),
        |v| v["references"][0]["concept_ids"] = serde_json::json!(["changed"]),
        |v| v["references"][0]["path"] = "../outside.fa".into(),
        |v| v["references"][0]["sha256"] = "a".repeat(64).into(),
        |v| v["licence"]["sha256"] = "a".repeat(64).into(),
    ];
    for edit in edits {
        let mut prepared = Prepared::new(Rescue);
        prepared.edit_manifest(edit);
        let output = prepared.command().output().unwrap();
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(!output.status.success(), "{report}");
        assert_eq!(
            report["anim_execution"]["stop_reason"], "component_integrity",
            "{report}"
        );
        assert_eq!(report["counts"]["attempted"], 0);
        assert_eq!(report["species_id"], Value::Null);
        assert_eq!(report["execution"]["status"], "incomplete");
    }
    for (path, contents) in [
        ("sequences/4.fa", b"altered".as_slice()),
        ("extra", b"unlisted"),
    ] {
        let prepared = Prepared::new(Rescue);
        std::fs::write(prepared.dir.path().join("fastas").join(path), contents).unwrap();
        let output = prepared.command().output().unwrap();
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(!output.status.success());
        assert_eq!(report["counts"]["attempted"], 0);
        assert_eq!(report["species_id"], Value::Null);
    }
}

#[test]
fn malformed_native_reference_is_failed_not_a_successful_no_alignment() {
    let prepared = Prepared::new(common::assembly::NativeCase::MalformedReference);
    let output = prepared.command().output().unwrap();
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!output.status.success(), "{report}");
    assert_eq!(report["counts"]["attempted"], 1);
    assert_eq!(report["counts"]["failed"], 1);
    assert_eq!(report["counts"]["no_alignment"], 0);
    assert_eq!(report["species_id"], Value::Null);
    assert_eq!(report["execution"]["status"], "incomplete");
    assert!(
        report["anim_execution"]["pairs"][0]["record"]["error"]
            .as_str()
            .unwrap()
            .contains("duplicate"),
        "{report}"
    );
    assert!(prepared
        .dir
        .path()
        .join("receipts/query-000001/pair-000001/result.json")
        .is_file());
}

#[test]
fn decisive_foreign_parent_veto_stops_before_unnecessary_pairs() {
    let prepared = Prepared::new(common::assembly::NativeCase::ForeignChallenge);
    let output = prepared.command().output().unwrap();
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(output.status.success(), "{report}");
    assert_eq!(
        report["anim_execution"]["stop_reason"], "decisive_veto",
        "{report}"
    );
    assert_eq!(report["species_id"], Value::Null);
    assert!(report["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "foreign_parent_challenge"));
    assert_eq!(report["counts"]["attempted"], 4);
    assert_eq!(report["counts"]["completed"], 4);
    assert_eq!(report["counts"]["not_run"], 1);
    assert_eq!(
        report["anim_execution"]["not_run_reference_ids"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn query_budget_retains_partial_receipts_and_returns_nonzero() {
    let prepared = Prepared::new(common::assembly::NativeCase::Rescue);
    let output = prepared
        .command()
        .args([
            "--anim-pair-seconds",
            "120",
            "--anim-query-seconds",
            "0.000000001",
        ])
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!output.status.success(), "{report}");
    assert_eq!(report["execution"]["status"], "incomplete");
    assert!(
        report["counts"]["completed"].as_u64().unwrap()
            < report["counts"]["required"].as_u64().unwrap(),
        "{report}"
    );
    assert_eq!(report["species_id"], Value::Null);
    assert!(
        report["execution"]["error"]
            .as_str()
            .unwrap()
            .contains("budget")
            || report["execution"]["error"]
                .as_str()
                .unwrap()
                .contains("deadline"),
        "{report}"
    );
    assert_eq!(report["counts"]["attempted"], 0);
    assert!(prepared
        .dir
        .path()
        .join("receipts/query-000001/query.fa")
        .exists());
    for pair in report["anim_execution"]["pairs"].as_array().unwrap() {
        assert!(std::path::Path::new(pair["directory"].as_str().unwrap())
            .join("result.json")
            .exists());
    }
}

#[test]
fn interrupted_native_run_retains_partial_evidence_and_withholds() {
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };
    let prepared = Prepared::new(common::assembly::NativeCase::Rescue);
    let command = prepared.command();
    let mut child = std::process::Command::new(command.get_program())
        .args(command.get_args())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let request = prepared
        .dir
        .path()
        .join("receipts/query-000001/pair-000001/QR/request.json");
    let start = Instant::now();
    while !request.exists() && start.elapsed() < Duration::from_secs(20) {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    unsafe {
        libc::kill(child.id() as i32, libc::SIGINT);
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        request.exists(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!output.status.success(), "{report}");
    assert_eq!(
        report["anim_execution"]["stop_reason"], "cancelled",
        "{report}"
    );
    assert_eq!(report["species_id"], Value::Null);
    assert_eq!(report["counts"]["attempted"], 1);
    assert_eq!(report["counts"]["failed"], 1);
    assert_eq!(report["execution"]["status"], "incomplete");
}

#[test]
fn existing_output_is_preserved_and_v1_rejects_anim_options() {
    let prepared = Prepared::new(common::assembly::NativeCase::Rescue);
    let output_dir = prepared.dir.path().join("receipts");
    std::fs::create_dir(&output_dir).unwrap();
    std::fs::write(output_dir.join("old"), "preserve").unwrap();
    let output = prepared.command().output().unwrap();
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!output.status.success());
    assert_eq!(report["execution"]["stage"], "anim_output");
    assert_eq!(
        std::fs::read_to_string(output_dir.join("old")).unwrap(),
        "preserve"
    );
    let output = cargo_bin_cmd!("terobacter")
        .arg("--reference-package")
        .arg(prepared.dir.path().join("package"))
        .args([
            "--expected-sha256",
            &prepared.pin,
            "--anim-memory-mib",
            "64",
        ])
        .arg(&prepared.query)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(5));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["execution"]["stage"], "policy");
    assert_eq!(report["assignment"]["species_id"], Value::Null);
}

#[test]
fn v2_aliases_preserve_sequential_batch_status_without_fallback() {
    let prepared = Prepared::new(common::assembly::NativeCase::Rescue);
    let bad = prepared.dir.path().join("bad.fa");
    std::fs::write(&bad, "not FASTA").unwrap();
    let mut reports = Vec::new();
    for program in [
        assert_cmd::cargo::cargo_bin!("tero"),
        assert_cmd::cargo::cargo_bin!("terobacter"),
    ] {
        let output = std::process::Command::new(program)
            .arg("--reference-package")
            .arg(prepared.dir.path().join("package"))
            .args([
                "--expected-sha256",
                &prepared.pin,
                "--policy",
                "conservative-assembly-anim-v2",
            ])
            .args([&prepared.query, &bad])
            .output()
            .unwrap();
        assert!(!output.status.success());
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report[0]["trigger"], true);
        assert_eq!(report[0]["execution"]["stage"], "anim");
        assert_eq!(report[1]["execution"]["stage"], "input");
        for result in report.as_array().unwrap() {
            assert_eq!(result["policy_id"], "conservative-assembly-anim-v2");
            assert_eq!(result["execution"]["status"], "incomplete");
        }
        let mut report = report;
        report[0]["anim_execution"]
            .as_object_mut()
            .unwrap()
            .remove("wall_seconds");
        reports.push(report);
    }
    assert_eq!(reports[0], reports[1]);
}

#[test]
fn changed_component_after_a_completed_witness_cannot_publish_a_rescue() {
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };
    let prepared = Prepared::new(common::assembly::NativeCase::Rescue);
    let command = prepared.command();
    let mut child = std::process::Command::new(command.get_program())
        .args(command.get_args())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let second_pair = prepared
        .dir
        .path()
        .join("receipts/query-000001/pair-000002/QR/request.json");
    let start = Instant::now();
    while !second_pair.exists() && start.elapsed() < Duration::from_secs(30) {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let reached_second_pair = second_pair.exists();
    if reached_second_pair {
        std::fs::write(
            prepared.dir.path().join("fastas/sequences/0.fa"),
            "changed after witness",
        )
        .unwrap();
    } else {
        child.kill().ok();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        reached_second_pair,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!output.status.success(), "{report}");
    assert_eq!(
        report["anim_execution"]["stop_reason"], "component_recheck",
        "{report}"
    );
    assert_eq!(report["counts"]["completed"], 5);
    assert_eq!(report["species_id"], Value::Null);
    assert_eq!(report["subspecies_id"], Value::Null);
    assert_eq!(report["genomovar_id"], Value::Null);
    assert_eq!(report["rescue_status"], "incomplete");
    assert_eq!(report["execution"]["status"], "incomplete");
    assert_eq!(
        report["witness"]["reference_id"],
        "primary_whole:GCA_900000001.1"
    );
}

#[test]
fn pair_deadline_before_worker_start_is_failed_and_retained() {
    let prepared = Prepared::new(common::assembly::NativeCase::Rescue);
    let output = prepared
        .command()
        .args(["--anim-pair-seconds", "0.000000001"])
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!output.status.success(), "{report}");
    assert_eq!(report["counts"]["attempted"], 1);
    assert_eq!(report["counts"]["failed"], 1);
    assert_eq!(report["counts"]["no_alignment"], 0);
    assert_eq!(report["species_id"], Value::Null);
    assert!(report["anim_execution"]["pairs"][0]["record"]["error"]
        .as_str()
        .unwrap()
        .contains("deadline"));
}

#[test]
fn invalid_time_and_memory_limits_are_usage_errors() {
    let prepared = Prepared::new(common::assembly::NativeCase::Rescue);
    for (flag, value) in [
        ("--anim-pair-seconds", "1801"),
        ("--anim-query-seconds", "7201"),
        ("--anim-pair-seconds", "NaN"),
        ("--anim-query-seconds", "inf"),
        ("--anim-pair-seconds", "0"),
        ("--anim-query-seconds", "0.00000000001"),
        ("--anim-memory-mib", "63"),
        ("--anim-memory-mib", "6145"),
    ] {
        let output = prepared.command().args([flag, value]).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{flag} {value}");
        assert!(output.stdout.is_empty());
    }
}
