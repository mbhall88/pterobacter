//! Decision seam: the species gates withhold rather than assign.
//!
//! Evidence is built row by row so each test states exactly the comparison that matters.
//! Expected reasons are the frozen 0.2.0 reason strings.

mod common;

use common::{decision_for, evidence_json, row, Row};
use serde_json::Value;

fn status_and_id(decision: &Value) -> (&str, Option<&str>) {
    let species = &decision["species"];
    (species["status"].as_str().unwrap(), species["id"].as_str())
}

fn reason(rows: &[Row]) -> String {
    decision_for(rows)["species"]["reason"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn species_gates_are_inclusive_at_exactly_96_ani_and_80_both_af() {
    let decision = decision_for(&[
        row("ref-a", Some("SP1"), 96.0, 80.0, 80.0),
        row("ref-b", Some("SP2"), 90.0, 90.0, 90.0),
    ]);

    assert_eq!(status_and_id(&decision), ("assigned", Some("SP1")));
}

#[test]
fn nearest_below_96_is_withheld() {
    let rows = [
        row("ref-a", Some("SP1"), 95.99, 90.0, 90.0),
        row("ref-b", Some("SP2"), 90.0, 90.0, 90.0),
    ];

    assert_eq!(status_and_id(&decision_for(&rows)), ("unresolved", None));
    assert_eq!(reason(&rows), "nearest_below_species_ANI");
}

#[test]
fn either_aligned_fraction_below_80_is_withheld() {
    for (aq, ar) in [(79.99, 90.0), (90.0, 79.99)] {
        let rows = [
            row("ref-a", Some("SP1"), 99.0, aq, ar),
            row("ref-b", Some("SP2"), 90.0, 90.0, 90.0),
        ];

        assert_eq!(reason(&rows), "species_coverage_not_met", "AF {aq}/{ar}");
    }
}

#[test]
fn a_qualifying_reference_other_than_the_nearest_can_witness_the_species() {
    let decision = decision_for(&[
        row("ref-a", Some("SP1"), 99.0, 50.0, 50.0),
        row("ref-b", Some("SP1"), 98.0, 85.0, 85.0),
        row("ref-c", Some("SP2"), 90.0, 90.0, 90.0),
    ]);

    assert_eq!(status_and_id(&decision), ("assigned", Some("SP1")));
    assert_eq!(decision["species"]["witness"]["reference_id"], "ref-b");
}

#[test]
fn a_tie_between_species_at_the_top_is_withheld() {
    let rows = [
        row("ref-a", Some("SP1"), 98.0, 90.0, 90.0),
        row("ref-b", Some("SP2"), 98.0, 90.0, 90.0),
    ];

    assert_eq!(reason(&rows), "different_species_nearest_tie");
}

#[test]
fn a_tie_with_an_unmapped_reference_is_a_tie_between_parents() {
    let rows = [
        row("ref-a", Some("SP1"), 98.0, 90.0, 90.0),
        row("ref-b", None, 98.0, 90.0, 90.0),
    ];

    assert_eq!(reason(&rows), "different_species_nearest_tie");
}

#[test]
fn unmapped_references_are_never_merged_into_one_parent() {
    let rows = [
        row("ref-a", None, 98.0, 90.0, 90.0),
        row("ref-b", None, 98.0, 90.0, 90.0),
    ];

    assert_eq!(reason(&rows), "different_species_nearest_tie");
}

#[test]
fn a_nearest_reference_without_a_species_is_withheld() {
    let rows = [
        row("ref-a", None, 98.0, 90.0, 90.0),
        row("ref-b", Some("SP2"), 90.0, 90.0, 90.0),
    ];

    assert_eq!(reason(&rows), "nearest_reference_has_no_species");
}

#[test]
fn a_panel_with_no_measured_competitor_is_withheld() {
    let rows = [
        row("ref-a", Some("SP1"), 99.0, 90.0, 90.0),
        row("ref-b", Some("SP1"), 98.0, 90.0, 90.0),
    ];

    assert_eq!(reason(&rows), "species_competitor_unmeasured");
}

#[test]
fn a_lineage_only_reference_counts_as_a_measured_competitor() {
    let decision = decision_for(&[
        row("ref-a", Some("SP1"), 99.0, 90.0, 90.0),
        row("ref-b", None, 90.0, 90.0, 90.0).lineage("LIN1"),
    ]);

    assert_eq!(status_and_id(&decision), ("assigned", Some("SP1")));
}

#[test]
fn an_estimate_free_panel_is_withheld_as_no_estimate() {
    let mut evidence = evidence_json(&[row("ref-a", Some("SP1"), 99.0, 90.0, 90.0)]);
    evidence["ranked_reference_evidence"] = Value::Array(vec![]);
    evidence["evidence_status"] = "no_estimate_returned".into();
    evidence["nearest_reference_ids"] = Value::Array(vec![]);
    evidence["unique_hit_count"] = 0.into();
    evidence["raw_hit_count"] = 0.into();
    evidence["not_returned_reference_count"] = 1.into();

    let decision = common::decision_from_text(&evidence.to_string());

    assert_eq!(decision["species"]["reason"], "no_estimate");
    assert_eq!(status_and_id(&decision), ("unresolved", None));
}

#[test]
fn an_assignment_lists_the_v1_checks_this_build_has_not_applied() {
    let decision = decision_for(&[
        row("ref-a", Some("SP1"), 99.0, 90.0, 90.0),
        row("ref-b", Some("SP2"), 97.0, 90.0, 85.0),
    ]);

    // Frozen v1 withholds this pair as a two-parent alert; the tracer does not check it yet,
    // so it must not look like a complete v1 result.
    let not_applied: Vec<_> = decision["v1_checks_not_applied"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap())
        .collect();
    assert!(
        not_applied.contains(&"two_parent_support_alert"),
        "{not_applied:?}"
    );
}
