//! Genomovar reporting in the whole-assembly workflow: the uncapped genomovar search of the
//! historical chromosome component, the assessment, and the publication recheck that clears a
//! genomovar whose parent no longer stands.
//!
//! The synthetic package's genomovars are not the frozen definition, so these tests pass the
//! fixture's own groups under the frozen version, and a v1-shaped Group A parent. The parent
//! bridge itself is tested with real decisions in `genomovar_parent.rs`.
mod common;

use std::collections::BTreeMap;

use assert_cmd::cargo::cargo_bin_cmd;
use common::native;
use common::package::sha256_hex;
use serde_json::{json, Value};
use terobacter::cli::assembly::{attach_result, invalidate, AssemblyWorkflow};
use terobacter::genomovar::{recheck, Definition, DEFINITION_VERSION, SEARCH_PARAMETERS};

const GV01: &str = "ECTX-GV-2026-01-01-v1-GV01";
const GV02: &str = "ECTX-GV-2026-01-01-v1-GV02";

fn member(accession: &str) -> String {
    format!("genomovar_chromosome:{accession}")
}

/// The fixture's genomovar groups: GV01 = {002, 006}, GV02 = {003}, GV03 = {007}.
fn definition() -> Definition {
    let groups = BTreeMap::from([
        (
            GV01.to_owned(),
            vec![member("GCA_900000002.1"), member("GCA_900000006.1")],
        ),
        (GV02.to_owned(), vec![member("GCA_900000003.1")]),
        (
            "ECTX-GV-2026-01-01-v1-GV03".to_owned(),
            vec![member("GCA_900000007.1")],
        ),
    ]);
    Definition::new(DEFINITION_VERSION, groups).unwrap()
}

/// GV01's members and the first primary reference are the base sequence; `also` names other
/// references that are too. Everything else differs from it at about 8% of positions.
fn package(also: &'static [&'static str]) -> (tempfile::TempDir, String) {
    native::package(move |id| {
        let base = native::base();
        let same = [
            "primary_whole:GCA_900000001.1",
            "genomovar_chromosome:GCA_900000002.1",
            "genomovar_chromosome:GCA_900000006.1",
        ];
        if same.contains(&id) || also.contains(&id) {
            base
        } else {
            native::mutated(&base, 8, 927)
        }
    })
}

fn query() -> Vec<u8> {
    [b">query\n".as_slice(), &native::base(), b"\n"].concat()
}

/// A final v1 report whose call is the Group A parent, bound to `pin` and the query bytes.
fn group_a_report(pin: &str, bytes: &[u8]) -> Value {
    json!({"policy": {"id": "conservative-assembly-v1"},
        "assignment": {"status": "assigned", "species_id": "ECTX-SP025",
            "subspecies_id": "ECTX-SS007", "genomovar_id": null},
        "blocking_reasons": [], "evaluation_status": "completed",
        "reference_package": {"status": "verified", "package": {"manifest_sha256": pin}},
        "input": {"kind": "assembly", "sequence_scope": "whole_assembly_as_supplied",
            "query_sha256": sha256_hex(bytes), "reference_package_sha256": pin},
        "scope_assessment": {"outcome": "not_assessed"}, "execution": {"status": "complete"}})
}

fn analyse(also: &'static [&'static str], edit: impl FnOnce(&mut Value)) -> Value {
    let (dir, pin) = package(also);
    let workflow = AssemblyWorkflow::open(&dir.path().join("package"), &pin).unwrap();
    let bytes = query();
    let mut report = group_a_report(&pin, &bytes);
    edit(&mut report);
    report["genomovar"] = workflow
        .genomovar(&definition(), "query.fa", &bytes, 2, &report)
        .unwrap();
    report
}

#[test]
fn a_unique_group_is_assigned_from_the_native_genomovar_search() {
    let mut report = analyse(&[], |_| {});
    let section = &report["genomovar"];
    assert_eq!(section["status"], "assigned", "{section}");
    assert_eq!(section["genomovar_id"], GV01);
    assert_eq!(section["search"]["status"], "completed");
    assert_eq!(section["search"]["parameters"], SEARCH_PARAMETERS);
    assert_eq!(
        section["search"]["reference_scope"],
        "historical_chromosome_record_set"
    );
    assert_eq!(
        section["search"]["query_scope"],
        "whole_assembly_as_supplied"
    );
    assert_eq!(section["search"]["reference_count"], 4);
    // The rival singletons are excluded by measured below-threshold members.
    assert_eq!(
        section["groups"][1]["below_threshold"],
        json!([member("GCA_900000003.1")])
    );
    assert_eq!(section["groups"][1]["state"], "incompatible");
    // The report-level call is set only at publication, after the parent is rechecked.
    assert_eq!(report["assignment"]["genomovar_id"], Value::Null);
    recheck(&mut report);
    assert_eq!(report["assignment"]["genomovar_id"], GV01);
}

#[test]
fn an_overlapping_definition_is_withheld_with_both_groups_kept() {
    let mut report = analyse(&["genomovar_chromosome:GCA_900000003.1"], |_| {});
    let section = &report["genomovar"];
    assert_eq!(section["status"], "withheld");
    assert_eq!(section["compatible_groups"], json!([GV01, GV02]));
    assert!(section["reasons"]
        .as_array()
        .unwrap()
        .contains(&json!("overlapping_definitions")));
    recheck(&mut report);
    assert_eq!(report["assignment"]["genomovar_id"], Value::Null);
}

#[test]
fn an_ineligible_parent_runs_no_genomovar_search() {
    for edit in [
        (|r: &mut Value| r["assignment"]["subspecies_id"] = "ECTX-SS001".into()) as fn(&mut Value),
        |r| r["assignment"]["status"] = "unresolved".into(),
        |r| r["blocking_reasons"] = json!(["two_parent_support_alert"]),
    ] {
        let report = analyse(&[], edit);
        let section = &report["genomovar"];
        assert_eq!(section["status"], "withheld", "{section}");
        assert_eq!(section["search"]["status"], "not_run");
        assert_eq!(section["genomovar_id"], Value::Null);
        assert_eq!(section["groups"], json!([]));
    }
}

#[test]
fn a_parent_bound_to_another_query_is_not_a_parent() {
    let report = analyse(&[], |r| r["input"]["query_sha256"] = "b".repeat(64).into());
    let section = &report["genomovar"];
    assert_eq!(section["status"], "withheld");
    assert!(section["reasons"]
        .as_array()
        .unwrap()
        .contains(&json!("incomplete_or_unbound_execution")));
}

#[test]
fn a_late_parent_veto_clears_the_genomovar_before_publication() {
    let late: [fn(&mut Value); 4] = [
        |r| {
            r["assignment"]["status"] = "unresolved".into();
            r["blocking_reasons"] = json!(["external_problem_warning"]);
        },
        |r| r["assignment"]["subspecies_id"] = Value::Null,
        |r| r["execution"] = json!({"status": "incomplete", "stage": "package_recheck"}),
        |r| r["input"]["query_sha256"] = "c".repeat(64).into(),
    ];
    for veto in late {
        let mut report = analyse(&[], |_| {});
        assert_eq!(report["genomovar"]["status"], "assigned");
        veto(&mut report);
        recheck(&mut report);
        let section = &report["genomovar"];
        assert_eq!(report["assignment"]["genomovar_id"], Value::Null);
        assert_eq!(section["genomovar_id"], Value::Null);
        assert_eq!(section["status"], "withheld");
        assert!(section["reasons"]
            .as_array()
            .unwrap()
            .contains(&json!("parent_withdrawn_before_publication")));
        // The compatible evidence is kept.
        assert_eq!(section["compatible_groups"], json!([GV01]));
    }
}

#[test]
fn an_invalidated_report_publishes_neither_its_call_nor_its_genomovar() {
    // A primary integrity failure withdraws all dependent assignments.
    {
        let stage = "package_recheck";
        let mut report = analyse(&[], |_| {});
        report["species"] = json!({"id": "ECTX-SP025"});
        report["subspecies"] = json!({"id": "ECTX-SS007"});
        report["evidence"] = json!({"kept": true});
        invalidate(&mut report, stage, "failed");
        recheck(&mut report);
        assert_eq!(
            report["execution"],
            json!({"status": "incomplete", "stage": stage, "error": "failed"})
        );
        assert_eq!(report["assignment"]["status"], "not_evaluated");
        for field in ["species_id", "subspecies_id", "genomovar_id"] {
            assert_eq!(report["assignment"][field], Value::Null, "{field}");
        }
        assert_eq!(report["species"], Value::Null);
        assert_eq!(report["subspecies"], Value::Null);
        assert_eq!(report["evidence"], json!({"kept": true}));
        assert_eq!(report["genomovar"]["genomovar_id"], Value::Null);
        assert!(report["genomovar"]["reasons"]
            .as_array()
            .unwrap()
            .contains(&json!("parent_withdrawn_before_publication")));
    }
}

#[test]
fn the_public_command_reports_a_withheld_genomovar_for_a_package_without_the_definition() {
    let (dir, pin) = package(&[]);
    let path = dir.path().join("query.fa");
    std::fs::write(&path, query()).unwrap();
    let output = cargo_bin_cmd!("tero")
        .arg("--reference-package")
        .arg(dir.path().join("package"))
        .args(["--expected-sha256", &pin])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["execution"]["status"], "complete");
    assert_eq!(report["assignment"]["species_id"], "SYN-SP01");
    assert_eq!(report["assignment"]["genomovar_id"], Value::Null);
    let section = &report["genomovar"];
    assert_eq!(section["status"], "withheld");
    assert_eq!(section["search"]["status"], "not_run");
    let reasons = section["reasons"].as_array().unwrap();
    assert!(reasons.contains(&json!("absent_eligible_parent")));
    assert!(reasons.contains(&json!("unsupported_method_or_definition")));
    assert!(section["detail"]
        .as_str()
        .unwrap()
        .contains("frozen genomovar definition"));
}

#[test]
fn supplemental_failure_retains_parent_but_late_package_failure_withdraws_it() {
    for v2 in [false, true] {
        let (dir, pin) = package(&[]);
        let workflow = AssemblyWorkflow::open(&dir.path().join("package"), &pin).unwrap();
        let mut report = group_a_report(&pin, &query());
        if v2 {
            report["policy_id"] = "conservative-assembly-anim-v2".into();
            report["policy"] = Value::Null;
            report["status"] = "assigned".into();
            report["species_id"] = "ECTX-SP025".into();
            report["subspecies_id"] = "ECTX-SS007".into();
            report["assignment"] = Value::Null;
        }
        let original = report.clone();
        assert!(attach_result(
            &mut report,
            Err("synthetic standalone search failure".into())
        )
        .is_err());
        recheck(&mut report);
        assert_eq!(report["genomovar"]["status"], "failed");
        assert_eq!(report["execution"]["stage"], "genomovar");
        assert_eq!(report["execution"]["status"], "incomplete");
        assert_eq!(report["assignment"], original["assignment"]);
        assert_eq!(report["species_id"], original["species_id"]);
        assert_eq!(report["subspecies_id"], original["subspecies_id"]);
        workflow.finish().unwrap();
        std::fs::write(dir.path().join("package/portable.json"), b"changed").unwrap();
        let error = workflow.finish().unwrap_err();
        invalidate(&mut report, "package_recheck", &error);
        recheck(&mut report);
        let assignment = if v2 { &report } else { &report["assignment"] };
        assert_eq!(assignment["species_id"], Value::Null);
        assert_eq!(assignment["subspecies_id"], Value::Null);
        assert_eq!(report["execution"]["stage"], "package_recheck");
    }
}

#[test]
fn emitted_provenance_binds_contract_parent_policy_and_decision() {
    use terobacter::genomovar::{
        parent_policy_sha256, parent_result_sha256, reporting_policy_sha256,
    };
    let mut report = analyse(&[], |_| {});
    let section = report["genomovar"].clone();
    assert_eq!(section["policy_sha256"], reporting_policy_sha256());
    assert_eq!(section["approval"]["status"], "approved");
    assert_eq!(section["approval"]["by"], "Michael");
    assert_eq!(
        section["parent"]["policy_sha256"],
        parent_policy_sha256("conservative-assembly-v1")
    );
    assert_eq!(
        section["parent"]["result_sha256"],
        parent_result_sha256(&report)
    );
    let hash = parent_result_sha256(&report);
    recheck(&mut report);
    assert_eq!(hash, parent_result_sha256(&report));
    report["blocking_reasons"] = json!(["late_veto"]);
    assert_ne!(hash, parent_result_sha256(&report));
    recheck(&mut report);
    assert_eq!(report["assignment"]["genomovar_id"], Value::Null);
    assert_eq!(
        report["genomovar"]["compatible_groups"],
        section["compatible_groups"]
    );
}

#[test]
fn independent_mlst_receipts_preserve_genomovar_but_parent_changes_still_veto() {
    use terobacter::genomovar::parent_result_sha256;

    let original = analyse(&[], |_| {});
    assert_eq!(original["genomovar"]["status"], "assigned");
    let hash = parent_result_sha256(&original);
    for mlst in [
        json!({"status": "not_requested"}),
        json!({"status": "assigned", "sequence_type": "42", "scheme": "synthetic"}),
        json!({"status": "failed", "detail": "synthetic standalone typing failure"}),
    ] {
        let mut report = original.clone();
        report["mlst"] = mlst;
        assert_eq!(parent_result_sha256(&report), hash);
        recheck(&mut report);
        assert_eq!(report["assignment"]["genomovar_id"], GV01);
        assert_eq!(report["genomovar"]["status"], "assigned");

        report["blocking_reasons"] = json!(["late_parent_veto"]);
        assert_ne!(parent_result_sha256(&report), hash);
        recheck(&mut report);
        assert_eq!(report["assignment"]["genomovar_id"], Value::Null);
        assert_eq!(report["genomovar"]["status"], "withheld");
        assert_eq!(
            report["genomovar"]["compatible_groups"],
            original["genomovar"]["compatible_groups"]
        );
    }
}
