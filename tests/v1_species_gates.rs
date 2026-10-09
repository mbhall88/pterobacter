//! Decision seam: the species gates withhold rather than assign.
//!
//! Evidence is built row by row so each test states exactly the comparison that matters.
//! Expected reasons are the frozen 0.2.0 reason strings.

mod common;

use common::{
    decision_for, decision_from_text, evidence_json, evidence_text_with_literals, row, Row,
};
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
fn two_decimal_literals_beside_the_thresholds_decide_exactly() {
    let rows = [
        row("ref-a", Some("SP1"), 96.0, 80.0, 80.0),
        row("ref-b", Some("SP2"), 90.0, 90.0, 90.0),
    ];
    let assigned = "conditional_assembly_assignment";
    let below_ani = "nearest_below_species_ANI";
    let below_af = "species_coverage_not_met";
    // The minimum aligned fraction is restated wherever a part drops below its base value of 80.
    let cases: [(&[(&str, &str)], &str); 7] = [
        (&[("ani_percent", "95.99")], below_ani),
        (&[("ani_percent", "96.00")], assigned),
        (&[("ani_percent", "96")], assigned),
        (
            &[
                ("aligned_fraction_query_percent", "79.99"),
                ("minimum_directional_af_percent", "79.99"),
            ],
            below_af,
        ),
        (&[("aligned_fraction_query_percent", "80.00")], assigned),
        (
            &[
                ("aligned_fraction_reference_percent", "79.99"),
                ("minimum_directional_af_percent", "79.99"),
            ],
            below_af,
        ),
        (&[("aligned_fraction_reference_percent", "80")], assigned),
    ];

    for (overrides, expected) in cases {
        let decision = decision_from_text(&evidence_text_with_literals(&rows, 0, overrides));

        assert_eq!(decision["species"]["reason"], expected, "{overrides:?}");
    }
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

    assert_eq!(
        reason(&rows),
        "different_species_nearest_tie;two_parent_support_alert"
    );
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
fn two_named_parents_with_reference_af_support_withhold_even_with_low_query_af() {
    let decision = decision_for(&[
        row("ref-a", Some("SP1"), 99.0, 90.0, 90.0),
        row("ref-b", Some("SP2"), 96.0, 20.0, 80.0),
    ]);
    assert_eq!(status_and_id(&decision), ("unresolved", None));
    assert_eq!(
        decision["blocking_reasons"],
        serde_json::json!(["two_parent_support_alert"])
    );
    assert_eq!(decision["species"]["reason"], "two_parent_support_alert");
}
