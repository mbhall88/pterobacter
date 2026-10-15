//! TSV and Markdown reports must agree with the authoritative JSON report. The reports under
//! test are small synthetic documents in the shapes `ptero` emits for each policy.
mod common;

use common::p1;
use serde_json::{json, Value};
use terobacter::report::{annotate, render};

const QUERY_SHA: &str = "3fa07c219f750c844d195aaf6f5802748b9025ef5c9d0af7f7a0367d326e52ff";
const PACKAGE_SHA: &str = "84f87508cf27391c97ed753af11855955bda371b561c38e4aeb22c2c5bd0b459";

fn input(file: &str) -> Value {
    json!({"kind": "assembly", "file": file, "query_sha256": QUERY_SHA, "total_bases": 200008,
        "sequence_scope": "whole_assembly_as_supplied", "evidence_sha256": "ee".repeat(32),
        "records": [{"ordinal": 1, "header": "chr", "length": 200003, "sequence_sha256": "aa".repeat(32)},
            {"ordinal": 2, "header": "p1", "length": 5, "sequence_sha256": "bb".repeat(32)}]})
}

fn package() -> Value {
    json!({"status": "verified", "package": {"version": "2026-10-09.test2",
        "manifest_sha256": PACKAGE_SHA, "id": "synthetic-enterobacter-reference"}})
}

fn measurement() -> Value {
    json!({"engine_profile": "skani-0.3.2-avx2-compatible-dna-v1", "engine_version": "0.3.2"})
}

fn skani(ani: f64, af: f64) -> Value {
    json!({"ani_percent": ani, "af_query_percent": af, "af_reference_percent": af})
}

fn top(id: &str, species: &str, ani: f64, tie_group: u32) -> Value {
    json!({"reference_id": id, "tie_group": tie_group, "species_id": species,
        "species_name": format!("Enterobacter {}", species.to_lowercase()), "subspecies_id": null,
        "lineage_id": null, "reference_role": "type_anchor",
        "parent": {"kind": "species", "concept_id": species},
        "skani": skani(ani, 99.0), "anim": null})
}

fn v1_assigned() -> Value {
    json!({"schema": "terobacter.assembly-call/1",
        "implementation": {"name": "terobacter", "version": "0.1.0"},
        "policy": {"id": "conservative-assembly-v1"},
        "input": input("isolate.fa"),
        "execution": {"status": "complete", "stage": "genomovar"},
        "assignment": {"status": "assigned", "reason": "conditional_assembly_assignment",
            "species_id": "SYN-SP01", "species_name": "Enterobacter syntheticus",
            "subspecies_id": "SYN-SS01", "subspecies_name": "Enterobacter syntheticus subsp. syntheticus",
            "subspecies_reason": "diagnostic_candidate_only", "lineage_id": null, "genomovar_id": null},
        "blocking_reasons": [],
        "measurement": measurement(), "reference_package": package(),
        "thresholds": {"species_ani": "96", "subspecies_ani": "98", "both_af_minimum": "80",
            "subspecies_margin_percentage_points": "1", "two_parent_alert_ani": "96", "supplied": null},
        "evidence": {"reference_count": 5, "raw_hit_count": 5, "not_returned_reference_count": 0},
        "top_references": [top("primary_whole:A", "SYN-SP01", 100.0, 0), top("primary_whole:B", "SYN-SP02", 91.38, 1)],
        "panel_summary": [
            {"parent": {"kind": "species", "concept_id": "SYN-SP01"}, "panel_reference_count": 3,
             "returned_reference_count": 3, "best_skani_ani_percent": 100.0,
             "best_skani_reference_ids": ["primary_whole:A"], "in_top_references": true,
             "challenging_reference_ids": [], "anim_unmeasured_reference_count": null},
            {"parent": {"kind": "species", "concept_id": "SYN-SP02"}, "panel_reference_count": 2,
             "returned_reference_count": 2, "best_skani_ani_percent": 91.38,
             "best_skani_reference_ids": ["primary_whole:B"], "in_top_references": true,
             "challenging_reference_ids": [], "anim_unmeasured_reference_count": null}],
        "genomovar": {"status": "withheld", "genomovar_id": null, "reasons": ["absent_eligible_parent"]},
        "mlst": {"requested": false, "status": "not_requested"}})
}

fn counts(
    required: u32,
    attempted: u32,
    completed: u32,
    no_alignment: u32,
    failed: u32,
    not_run: u32,
) -> Value {
    json!({"unit": "reciprocal_pair", "required": required, "attempted": attempted,
        "completed": completed, "no_alignment": no_alignment, "failed": failed, "not_run": not_run,
        "missing_orientation": 0, "supplied_outside_plan": 0})
}

fn v2_base() -> Value {
    json!({"schema": "3.0.0-conditional-assembly-report",
        "implementation": {"name": "terobacter", "version": "0.1.0"},
        "policy_id": "conservative-assembly-anim-v2", "contract_revision": "P1-r1",
        "policy_mode": "default_candidate", "validation_status": "not_yet_validated",
        "thresholds": {"species_ani": "96", "subspecies_ani": "98", "both_af_minimum": "80",
            "subspecies_margin_percentage_points": "1", "conflict_ani": "96",
            "trigger_lower_ani": "94.5", "supplied": null},
        "input": input("borderline.fa"),
        "execution": {"status": "complete", "stage": "genomovar"},
        "measurement": measurement(), "reference_package": package(),
        "anim_execution": {"method_profile": "reciprocal-mummer4-4.0.1-preserve-n-mum-1-span-v1"},
        "legacy_v1_result": {"policy_id": "conservative-assembly-v1", "status": "unresolved",
            "species_id": null, "species_name": null, "reasons": ["nearest_below_species_ANI"],
            "assignment_reason": "nearest_below_species_ANI"},
        "max_skani_ani_percent": 95.29, "trigger": true,
        "trigger_state": "triggered_with_witness_candidate",
        "nearest_parent": {"kind": "species", "concept_id": "SYN-SP01", "name": "Enterobacter syntheticus"},
        "evidence": {"reference_count": 5, "raw_hit_count": 4, "not_returned_reference_count": 1},
        "genomovar_id": null,
        "mlst": {"requested": false, "status": "not_requested"}})
}

fn v2_rescued() -> Value {
    let mut r = v2_base();
    r["status"] = "assigned".into();
    r["species_id"] = "SYN-SP01".into();
    r["species_name"] = "Enterobacter syntheticus".into();
    r["subspecies_id"] = Value::Null;
    r["subspecies_name"] = Value::Null;
    r["subspecies_reason"] = "ANIm_species_rescue_does_not_establish_subspecies".into();
    r["call_basis"] = "ANIm_assisted".into();
    r["reasons"] = json!([]);
    r["rescue_status"] = "rescued".into();
    r["competitor_evaluation"] = "complete".into();
    r["evaluation_status"] = "complete".into();
    r["counts"] = counts(5, 5, 5, 1, 0, 0);
    r["challenges"] = json!([]);
    r["challenge_evidence"] = json!([]);
    r["top_references"] = json!([top("primary_whole:A", "SYN-SP01", 95.29, 0)]);
    r["panel_summary"] = json!([{"parent": {"kind": "species", "concept_id": "SYN-SP01"},
        "panel_reference_count": 3, "returned_reference_count": 3, "best_skani_ani_percent": 95.29,
        "best_skani_reference_ids": ["primary_whole:A"], "in_top_references": true,
        "challenging_reference_ids": [], "anim_unmeasured_reference_count": 0}]);
    r["genomovar"] =
        json!({"status": "withheld", "genomovar_id": null, "reasons": ["absent_eligible_parent"]});
    r
}

/// Withheld by a foreign-parent challenge that sits outside the displayed top references. The
/// historical scaffold still carries the label an earlier stage proposed.
fn v2_withheld_with_stale_scaffold() -> Value {
    let mut r = v2_base();
    r["status"] = "unresolved".into();
    r["species_id"] = Value::Null;
    r["species_name"] = Value::Null;
    r["subspecies_id"] = Value::Null;
    r["subspecies_name"] = Value::Null;
    r["subspecies_reason"] = "parent_species_unresolved".into();
    r["call_basis"] = Value::Null;
    r["reasons"] = json!(["ANIm_competitor_incomplete", "foreign_parent_challenge"]);
    r["rescue_status"] = "withheld".into();
    r["competitor_evaluation"] = "incomplete".into();
    r["evaluation_status"] = "incomplete".into();
    r["counts"] = counts(5, 4, 4, 0, 0, 1);
    r["challenges"] = json!(["primary_whole:Z"]);
    r["challenge_evidence"] = json!([{"reference_id": "primary_whole:Z",
        "parent": {"kind": "species", "concept_id": "SYN-SP09"}, "skani": skani(95.15, 99.31),
        "skani_challenges": false, "anim_challenging_orientations": ["QR", "RQ"]}]);
    r["top_references"] = json!([top("primary_whole:A", "SYN-SP01", 95.29, 0)]);
    r["panel_summary"] = json!([
        {"parent": {"kind": "species", "concept_id": "SYN-SP01"}, "panel_reference_count": 3,
         "returned_reference_count": 3, "best_skani_ani_percent": 95.29,
         "best_skani_reference_ids": ["primary_whole:A"], "in_top_references": true,
         "challenging_reference_ids": [], "anim_unmeasured_reference_count": 0},
        {"parent": {"kind": "species", "concept_id": "SYN-SP09"}, "panel_reference_count": 2,
         "returned_reference_count": 1, "best_skani_ani_percent": 95.15,
         "best_skani_reference_ids": ["primary_whole:Z"], "in_top_references": false,
         "challenging_reference_ids": ["primary_whole:Z"], "anim_unmeasured_reference_count": 1}]);
    // Stale scaffold: labels an earlier stage wrote, which the final decision withheld.
    r["assignment"] = json!({"status": "assigned", "species_id": "SYN-SP01",
        "species_name": "Enterobacter syntheticus", "subspecies_id": "SYN-SS01",
        "subspecies_name": "Enterobacter syntheticus subsp. syntheticus", "genomovar_id": "GV-STALE"});
    r["evidence"]["assignment"] = json!({"status": "assigned", "species_id": "SYN-SP01",
        "species_name": "Enterobacter syntheticus"});
    r["genomovar"] = json!({"status": "withheld", "genomovar_id": null,
        "reasons": ["absent_eligible_parent", "upstream_veto"]});
    r
}

fn partial_failure() -> Value {
    json!({"schema": "terobacter.assembly-call/1",
        "implementation": {"name": "terobacter", "version": "0.1.0"},
        "policy": {"id": "conservative-assembly-v1"},
        "input": {"kind": "assembly", "file": "bad\tname\n.fa", "sequence_scope": "whole_assembly_as_supplied"},
        "assignment": {"status": "not_evaluated", "species_id": null, "species_name": null,
            "subspecies_id": null, "subspecies_name": null, "lineage_id": null, "genomovar_id": null},
        "execution": {"status": "incomplete", "stage": "input", "error": "input must be assembly FASTA,\nnot FASTQ"},
        "mlst": {"requested": false, "status": "not_requested"}})
}

fn rows(tsv: &str) -> Vec<Vec<String>> {
    tsv.lines()
        .map(|l| l.split('\t').map(str::to_owned).collect())
        .collect()
}

/// The value of `column` in data row `row` of a rendered TSV.
fn cell(tsv: &str, row: usize, column: &str) -> String {
    let table = rows(tsv);
    let at = table[0]
        .iter()
        .position(|c| c == column)
        .unwrap_or_else(|| panic!("no column {column} in {:?}", table[0]));
    table[row + 1][at].clone()
}

#[test]
fn calls_tsv_has_one_row_per_assembly_and_agrees_with_a_v1_json_call() {
    let rendered = render(&[v1_assigned()]);
    assert_eq!(rows(&rendered.calls_tsv).len(), 2);
    for (column, expected) in [
        ("input_file", "isolate.fa"),
        ("query_sha256", QUERY_SHA),
        ("policy_id", "conservative-assembly-v1"),
        ("call_status", "assigned"),
        ("species_id", "SYN-SP01"),
        ("species_name", "Enterobacter syntheticus"),
        ("subspecies_id", "SYN-SS01"),
        ("execution_status", "complete"),
        ("skani_method", "skani-0.3.2-avx2-compatible-dna-v1"),
        ("implementation_version", "0.1.0"),
        ("package_manifest_sha256", PACKAGE_SHA),
        ("mlst_status", "not_requested"),
        ("min_species_ani", "96"),
        ("min_subspecies_ani", "98"),
        ("min_af", "80"),
        ("subspecies_margin", "1"),
        ("conflict_ani", ""),
        ("skani_references", "5"),
        ("skani_returned", "5"),
        ("skani_not_returned", "0"),
        ("anim_required", ""),
        ("original_v1_status", "assigned"),
        ("original_v1_species_id", "SYN-SP01"),
    ] {
        assert_eq!(cell(&rendered.calls_tsv, 0, column), expected, "{column}");
    }
}

#[test]
fn calls_tsv_agrees_with_a_v2_rescue_including_original_v1_outcome_and_counts() {
    let rendered = render(&[v2_rescued()]);
    for (column, expected) in [
        ("policy_id", "conservative-assembly-anim-v2"),
        ("validation_status", "not_yet_validated"),
        ("call_status", "assigned"),
        ("species_id", "SYN-SP01"),
        ("subspecies_id", ""),
        ("call_basis", "ANIm_assisted"),
        ("original_v1_status", "unresolved"),
        ("original_v1_species_id", ""),
        ("original_v1_reasons", "nearest_below_species_ANI"),
        ("trigger", "true"),
        ("rescue_status", "rescued"),
        ("competitor_evaluation", "complete"),
        ("anim_required", "5"),
        ("anim_attempted", "5"),
        ("anim_completed", "5"),
        ("anim_no_alignment", "1"),
        ("anim_failed", "0"),
        ("anim_not_run", "0"),
        ("min_species_ani", "96"),
        ("min_subspecies_ani", "98"),
        ("min_af", "80"),
        ("conflict_ani", "96"),
        ("trigger_lower_ani", "94.5"),
        (
            "anim_method",
            "reciprocal-mummer4-4.0.1-preserve-n-mum-1-span-v1",
        ),
        ("contract_revision", "P1-r1"),
    ] {
        assert_eq!(cell(&rendered.calls_tsv, 0, column), expected, "{column}");
    }
}

#[test]
fn a_withheld_call_never_takes_a_label_from_the_historical_scaffold() {
    let report = v2_withheld_with_stale_scaffold();
    let rendered = render(&[report]);
    for column in [
        "species_id",
        "species_name",
        "subspecies_id",
        "subspecies_name",
        "genomovar_id",
    ] {
        assert_eq!(cell(&rendered.calls_tsv, 0, column), "", "{column}");
    }
    assert_eq!(cell(&rendered.calls_tsv, 0, "call_status"), "unresolved");
    assert_eq!(
        cell(&rendered.calls_tsv, 0, "reasons"),
        "ANIm_competitor_incomplete;foreign_parent_challenge"
    );
    assert!(!rendered.calls_tsv.contains("GV-STALE"));
    assert!(!rendered.markdown.contains("GV-STALE"));
    // The proposed species may appear only as the nearest parent, never as the call.
    assert!(!rendered
        .markdown
        .contains("assigned — Enterobacter syntheticus"));
    assert!(rendered.markdown.contains("**Species:** unresolved"));
    assert!(rendered
        .markdown
        .contains("Nearest parent (not an assignment)"));
}

#[test]
fn markdown_states_the_same_final_facts_as_json() {
    for report in [
        v1_assigned(),
        v2_rescued(),
        v2_withheld_with_stale_scaffold(),
    ] {
        let rendered = render(std::slice::from_ref(&report));
        let md = &rendered.markdown;
        let policy = report["policy_id"]
            .as_str()
            .or(report["policy"]["id"].as_str())
            .unwrap();
        assert!(md.contains("Analysis settings and execution"), "{policy}");
        for forbidden in [
            "**Policy:**",
            "Policy mode",
            "Original v1",
            "conservative-assembly",
        ] {
            assert!(!md.contains(forbidden), "{md}");
        }
        assert!(md.contains(&format!("`{QUERY_SHA}`")));
        assert!(md.contains(PACKAGE_SHA));
        let status = cell(&rendered.calls_tsv, 0, "call_status");
        assert!(md.contains(&format!("**Call status:** {status}")));
        if status == "assigned" {
            let species = report["species_id"]
                .as_str()
                .or(report["assignment"]["species_id"].as_str())
                .unwrap();
            assert!(md.contains(&format!("({species})")));
            assert_eq!(cell(&rendered.calls_tsv, 0, "species_id"), species);
        }
        for reason in report["reasons"].as_array().into_iter().flatten() {
            assert!(md.contains(reason.as_str().unwrap()));
        }
        assert!(md.contains("Minimum species ANI"));
    }
}

#[test]
fn evidence_tsv_lists_top_references_tie_groups_and_every_panel_parent() {
    let mut report = v2_withheld_with_stale_scaffold();
    report["top_references"] = json!([
        top("primary_whole:A", "SYN-SP01", 95.29, 0),
        top("primary_whole:B", "SYN-SP01", 91.38, 1),
        top("primary_whole:C", "SYN-SP01", 91.38, 1)
    ]);
    let rendered = render(&[report]);
    let table = rows(&rendered.evidence_tsv);
    let header = &table[0];
    let at = |name: &str| header.iter().position(|c| c == name).unwrap();
    let tops: Vec<&Vec<String>> = table[1..]
        .iter()
        .filter(|r| r[at("section")] == "top_reference")
        .collect();
    assert_eq!(tops.len(), 3);
    assert_eq!(tops[1][at("tie_group")], tops[2][at("tie_group")]);
    assert_ne!(tops[0][at("tie_group")], tops[1][at("tie_group")]);
    assert_eq!(tops[0][at("skani_ani")], "95.29");
    let panel: Vec<&Vec<String>> = table[1..]
        .iter()
        .filter(|r| r[at("section")] == "panel_parent")
        .collect();
    assert_eq!(panel.len(), 2, "every panel parent is listed");
    let outside = panel
        .iter()
        .find(|r| r[at("parent_id")] == "SYN-SP09")
        .expect("parent outside the top references");
    assert_eq!(outside[at("in_top")], "false");
    assert_eq!(outside[at("challenging_reference_ids")], "primary_whole:Z");
    let challenges: Vec<&Vec<String>> = table[1..]
        .iter()
        .filter(|r| r[at("section")] == "challenge")
        .collect();
    assert_eq!(challenges.len(), 1);
    assert_eq!(challenges[0][at("reference_id")], "primary_whole:Z");
    assert_eq!(challenges[0][at("anim_orientations")], "QR;RQ");
}

#[test]
fn markdown_shows_a_challenge_outside_the_top_references_and_the_tie_rule() {
    let rendered = render(&[v2_withheld_with_stale_scaffold()]);
    let md = &rendered.markdown;
    assert!(md.contains("primary_whole:Z"), "challenge must be visible");
    assert!(md.contains("SYN-SP09"));
    assert!(md.contains("outside the strongest matches"));
    assert!(md.contains("exact tie at fifth place"));
    assert!(md.contains("ANI methods are not mixed"));
}

#[test]
fn batch_reports_get_one_row_each_and_failures_stay_visible() {
    let rendered = render(&[v1_assigned(), partial_failure(), v2_rescued()]);
    assert_eq!(rows(&rendered.calls_tsv).len(), 4);
    assert_eq!(
        cell(&rendered.calls_tsv, 1, "execution_status"),
        "incomplete"
    );
    assert_eq!(cell(&rendered.calls_tsv, 1, "execution_stage"), "input");
    assert_eq!(cell(&rendered.calls_tsv, 1, "call_status"), "not_evaluated");
    assert_eq!(cell(&rendered.calls_tsv, 1, "species_id"), "");
    assert_eq!(
        cell(&rendered.calls_tsv, 2, "policy_id"),
        "conservative-assembly-anim-v2"
    );
    assert!(rendered.markdown.contains("incomplete"));
    assert!(rendered.markdown.contains("Assembly 2"));
}

#[test]
fn tsv_cells_never_contain_tabs_or_newlines() {
    let rendered = render(&[partial_failure()]);
    for tsv in [&rendered.calls_tsv, &rendered.evidence_tsv] {
        let table = rows(tsv);
        let width = table[0].len();
        assert!(table.iter().all(|r| r.len() == width), "ragged TSV: {tsv}");
    }
    assert_eq!(cell(&rendered.calls_tsv, 0, "input_file"), "bad name .fa");
    assert_eq!(
        cell(&rendered.calls_tsv, 0, "execution_error"),
        "input must be assembly FASTA, not FASTQ"
    );
}

#[test]
fn annotate_adds_distinct_versions_without_changing_the_call() {
    for mut report in [v1_assigned(), v2_rescued()] {
        let before = report.clone();
        annotate(&mut report);
        let versions = &report["versions"];
        assert_eq!(versions["implementation"], "0.1.0");
        assert_eq!(versions["report_schema"], before["schema"]);
        assert!(versions["policy"].is_string());
        assert_eq!(
            versions["skani_method"],
            "skani-0.3.2-avx2-compatible-dna-v1"
        );
        assert_eq!(
            versions["reference_data"]["package_version"],
            "2026-10-09.test2"
        );
        assert_eq!(versions["reference_data"]["manifest_sha256"], PACKAGE_SHA);
        // Everything but the new blocks is untouched.
        report.as_object_mut().unwrap().remove("versions");
        assert_eq!(report, before);
    }
    let mut v2 = v2_rescued();
    annotate(&mut v2);
    assert_eq!(v2["versions"]["validation"], "not_yet_validated");
    assert_eq!(v2["versions"]["policy_contract"], "P1-r1");
    assert_eq!(
        v2["versions"]["anim_method"],
        "reciprocal-mummer4-4.0.1-preserve-n-mum-1-span-v1"
    );
    let mut v1 = v1_assigned();
    annotate(&mut v1);
    assert_eq!(v1["versions"]["validation"], Value::Null);
    assert_eq!(v1["versions"]["anim_method"], Value::Null);
}

#[test]
fn custom_thresholds_are_reported_as_custom_unvalidated_in_every_format() {
    let mut report = v2_rescued();
    report["policy_mode"] = "custom".into();
    report["validation_status"] = "custom_unvalidated".into();
    report["thresholds"]["species_ani"] = "95.5".into();
    report["thresholds"]["supplied"] = json!({"species_ani": "95.5"});
    let rendered = render(&[report]);
    assert_eq!(cell(&rendered.calls_tsv, 0, "policy_mode"), "custom");
    assert_eq!(
        cell(&rendered.calls_tsv, 0, "validation_status"),
        "custom_unvalidated"
    );
    assert_eq!(cell(&rendered.calls_tsv, 0, "min_species_ani"), "95.5");
    assert!(rendered.markdown.contains("custom_unvalidated"));
    assert!(rendered.markdown.contains("95.5"));
}

fn section_rows(tsv: &str, section: &str) -> Vec<Vec<String>> {
    let table = rows(tsv);
    let at = table[0].iter().position(|c| c == "section").unwrap();
    table[1..]
        .iter()
        .filter(|r| r[at] == section)
        .cloned()
        .collect()
}

fn column(tsv: &str, name: &str) -> usize {
    rows(tsv)[0].iter().position(|c| c == name).unwrap()
}

#[test]
fn every_exact_tie_at_fifth_place_is_listed_in_one_tie_group_by_the_production_decision() {
    let decision = p1::decide(&p1::case("display_fifth_tie_included")).unwrap();
    let rendered = render(std::slice::from_ref(&decision));
    let shown = section_rows(&rendered.evidence_tsv, "top_reference");
    let json_ids: Vec<String> = decision["top_references"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["reference_id"].as_str().unwrap().to_owned())
        .collect();
    let id = column(&rendered.evidence_tsv, "reference_id");
    let group = column(&rendered.evidence_tsv, "tie_group");
    assert_eq!(
        shown.iter().map(|r| r[id].clone()).collect::<Vec<_>>(),
        json_ids
    );
    assert!(json_ids.len() > 5, "the fifth-place tie extends the table");
    assert_eq!(shown[4][group], shown[5][group]);
    assert!(rendered.markdown.contains("exact tie at fifth place"));
    for reference in &json_ids {
        assert!(rendered.markdown.contains(reference.as_str()));
    }
}

#[test]
fn a_challenge_outside_the_top_five_reaches_every_format_from_the_production_decision() {
    let decision = p1::decide(&p1::case("competitor_outside_topfive_cannot_disappear")).unwrap();
    let rendered = render(std::slice::from_ref(&decision));
    let challenge = section_rows(&rendered.evidence_tsv, "challenge");
    let id = column(&rendered.evidence_tsv, "reference_id");
    assert!(challenge.iter().any(|r| r[id] == "B1"), "{challenge:?}");
    let parents = section_rows(&rendered.evidence_tsv, "panel_parent");
    let parent = column(&rendered.evidence_tsv, "parent_id");
    let in_top = column(&rendered.evidence_tsv, "in_top");
    let challenging = column(&rendered.evidence_tsv, "challenging_reference_ids");
    let b = parents.iter().find(|r| r[parent] == "B").expect("parent B");
    assert_eq!(b[in_top], "false");
    assert_eq!(b[challenging], "B1");
    assert!(rendered.markdown.contains("`B1`"));
    assert!(rendered.markdown.contains("outside the strongest matches"));
    assert_eq!(cell(&rendered.calls_tsv, 0, "call_status"), "unresolved");
    assert_eq!(cell(&rendered.calls_tsv, 0, "species_id"), "");
    assert!(cell(&rendered.calls_tsv, 0, "reasons").contains("foreign_parent_challenge"));
}

#[test]
fn production_decisions_agree_across_formats_for_every_p1_case() {
    for case in p1::cases().iter().filter(|c| c.get("raises").is_none()) {
        let name = case["id"].as_str().unwrap();
        let decision = p1::decide(case).unwrap();
        let rendered = render(std::slice::from_ref(&decision));
        assert_eq!(
            cell(&rendered.calls_tsv, 0, "call_status"),
            decision["status"],
            "{name}"
        );
        let species = decision["species_id"].as_str().unwrap_or_default();
        assert_eq!(
            cell(&rendered.calls_tsv, 0, "species_id"),
            species,
            "{name}"
        );
        let reasons: Vec<&str> = decision["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_str().unwrap())
            .collect();
        assert_eq!(
            cell(&rendered.calls_tsv, 0, "reasons"),
            reasons.join(";"),
            "{name}"
        );
        for key in [
            "required",
            "attempted",
            "completed",
            "no_alignment",
            "failed",
            "not_run",
        ] {
            assert_eq!(
                cell(&rendered.calls_tsv, 0, &format!("anim_{key}")),
                decision["counts"][key].to_string(),
                "{name} {key}"
            );
        }
        assert_eq!(
            section_rows(&rendered.evidence_tsv, "top_reference").len(),
            decision["top_references"].as_array().unwrap().len(),
            "{name}"
        );
        assert_eq!(
            section_rows(&rendered.evidence_tsv, "panel_parent").len(),
            decision["panel_summary"].as_array().unwrap().len(),
            "{name}"
        );
        assert_eq!(
            section_rows(&rendered.evidence_tsv, "challenge").len(),
            decision["challenge_evidence"].as_array().unwrap().len(),
            "{name}"
        );
        for challenge in decision["challenges"].as_array().unwrap() {
            assert!(
                rendered.markdown.contains(challenge.as_str().unwrap()),
                "{name}"
            );
        }
        if decision["status"] != "assigned" {
            assert!(!rendered.markdown.contains("assigned —"), "{name}");
        }
    }
}

#[test]
fn a_v1_report_says_which_sections_do_not_apply_instead_of_implying_a_clean_result() {
    let rendered = render(&[v1_assigned()]);
    let md = &rendered.markdown;
    assert!(md.contains("Initial skani result: this is the final result"));
    assert!(md.contains("ANIm: disabled"));
    assert!(md.contains("no alignment-based foreign-parent challenge test"));
    assert!(!md.contains("No reference challenged the call."));
}

#[test]
fn a_v2_report_with_no_challenge_still_says_none_challenged() {
    let rendered = render(&[v2_rescued()]);
    assert!(rendered
        .markdown
        .contains("No reference challenged the call."));
}

/// A missing measurement is not a negative result: coverage is independent of execution; a failed run must not imply a supported call.
#[test]
fn challenge_assessment_separates_unmeasured_partial_and_completed_checks() {
    let cases = [
        // (competitor_evaluation, execution status, assessment, markdown fragment)
        (
            "complete",
            "complete",
            "completed",
            "No reference challenged the call.",
        ),
        ("incomplete", "complete", "partial", "Partially assessed"),
        (
            "not_run_already_blocked",
            "complete",
            "not_assessed",
            "Not assessed",
        ),
        (
            "not_required_no_witness",
            "complete",
            "not_assessed",
            "Not assessed",
        ),
        (
            "not_required",
            "complete",
            "not_applicable",
            "Not applicable",
        ),
        ("incomplete", "incomplete", "partial", "Partially assessed"),
        (
            "complete",
            "incomplete",
            "completed",
            "execution or integrity checks failed",
        ),
    ];
    for (evaluation, execution, assessment, fragment) in cases {
        let mut report = v2_rescued();
        report["competitor_evaluation"] = evaluation.into();
        report["execution"]["status"] = execution.into();
        report["challenge_evidence"] = json!([]);
        let rendered = render(&[report]);
        assert_eq!(
            cell(&rendered.calls_tsv, 0, "challenge_assessment"),
            assessment,
            "{evaluation}/{execution}"
        );
        assert!(
            rendered.markdown.contains(fragment),
            "{evaluation}/{execution}: {}",
            rendered.markdown
        );
        assert_eq!(
            rendered
                .markdown
                .contains("No reference challenged the call."),
            assessment == "completed" && execution == "complete",
            "{evaluation}/{execution}"
        );
    }
}

#[test]
fn a_report_without_competitor_evidence_never_claims_an_unchallenged_call() {
    let mut report = v2_rescued();
    report
        .as_object_mut()
        .unwrap()
        .remove("competitor_evaluation");
    report["challenge_evidence"] = Value::Null;
    let rendered = render(&[report]);
    assert!(!rendered
        .markdown
        .contains("No reference challenged the call."));
    assert_eq!(
        cell(&rendered.calls_tsv, 0, "challenge_assessment"),
        "not_assessed"
    );
}

#[test]
fn incomplete_native_panel_retains_measured_challenge_and_counts() {
    let mut report = v2_withheld_with_stale_scaffold();
    report["execution"] =
        json!({"status": "incomplete", "stage": "decision", "error": "ANIm panel incomplete"});
    let rendered = render(&[report]);
    assert_eq!(
        cell(&rendered.calls_tsv, 0, "challenge_assessment"),
        "partial"
    );
    assert_eq!(
        cell(&rendered.calls_tsv, 0, "execution_status"),
        "incomplete"
    );
    assert_eq!(cell(&rendered.calls_tsv, 0, "anim_completed"), "4");
    assert_eq!(cell(&rendered.calls_tsv, 0, "anim_required"), "5");
    assert_eq!(cell(&rendered.calls_tsv, 0, "species_id"), "");
    assert!(rendered.markdown.contains("primary_whole:Z"));
    assert!(rendered.markdown.contains("Partially assessed"));
    assert!(!rendered.markdown.contains("Not assessed: the run stopped"));
    assert!(!rendered
        .markdown
        .contains("No reference challenged the call"));
}
