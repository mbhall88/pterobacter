//! Decision seam: the species gates withhold rather than assign.
//!
//! Evidence is built row by row so each test states exactly the comparison that matters.
//! Expected reasons are the frozen 0.2.0 reason strings.

use serde_json::{json, Value};
use terobacter::decision::{decide, Policy};
use terobacter::evidence::SavedEvidence;

/// (reference ID, species, ANI, query AF, reference AF); a `None` species is unmapped.
type Row = (&'static str, Option<&'static str>, f64, f64, f64);

fn evidence_json(rows: &[Row]) -> Value {
    let mut ranked = rows.to_vec();
    ranked.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(b.0)));
    let nearest: Vec<_> = ranked
        .iter()
        .take_while(|r| r.2 == ranked[0].2)
        .map(|r| r.0)
        .collect();
    let references: Vec<Value> = ranked
        .iter()
        .map(|&(id, species, ani, aq, ar)| {
            json!({
                "reference_id": id,
                "species_id": species.unwrap_or(""),
                "species_name": species.map_or(String::new(), |s| format!("Enterobacter {s}")),
                "lineage_id": "",
                "ani_percent": ani,
                "aligned_fraction_query_percent": aq,
                "aligned_fraction_reference_percent": ar,
                "minimum_directional_af_percent": aq.min(ar),
            })
        })
        .collect();
    json!({
        "schema_version": "2.0.0-dev-evidence",
        "record_type": "development_evidence_only",
        "input_kind": "assembly",
        "units": {"ani": "percent", "aligned_fraction": "percent"},
        "assignment": {"status": "not_evaluated"},
        "provenance": {
            "input_kind": "assembly",
            "engine_version": "0.3.2",
            "reference_component_id": "primary_whole",
            "reference_package_sha256": "a".repeat(64),
            "query_sha256": "b".repeat(64),
            "search_exit_code": 0,
        },
        "evidence_status": "estimates_returned",
        "reference_count": rows.len(),
        "raw_hit_count": rows.len(),
        "unique_hit_count": rows.len(),
        "not_returned_reference_count": 0,
        "duplicate_rows_collapsed": 0,
        "nearest_reference_ids": nearest,
        "ranked_reference_evidence": references,
    })
}

fn species_call(rows: &[Row]) -> (String, Option<String>) {
    let evidence = SavedEvidence::from_json(&evidence_json(rows).to_string()).unwrap();
    let decision: Value =
        serde_json::from_str(&decide(&evidence, &Policy::v1()).to_json()).unwrap();
    let species = &decision["species"];
    (
        species["status"].as_str().unwrap().to_owned(),
        species["id"].as_str().map(str::to_owned),
    )
}

fn reason(rows: &[Row]) -> String {
    let evidence = SavedEvidence::from_json(&evidence_json(rows).to_string()).unwrap();
    let decision: Value =
        serde_json::from_str(&decide(&evidence, &Policy::v1()).to_json()).unwrap();
    decision["species"]["reason"].as_str().unwrap().to_owned()
}

#[test]
fn species_gates_are_inclusive_at_exactly_96_ani_and_80_both_af() {
    let rows = [
        ("ref-a", Some("SP1"), 96.0, 80.0, 80.0),
        ("ref-b", Some("SP2"), 90.0, 90.0, 90.0),
    ];

    assert_eq!(
        species_call(&rows),
        ("assigned".to_owned(), Some("SP1".to_owned()))
    );
}

#[test]
fn nearest_below_96_is_withheld() {
    let rows = [
        ("ref-a", Some("SP1"), 95.99, 90.0, 90.0),
        ("ref-b", Some("SP2"), 90.0, 90.0, 90.0),
    ];

    assert_eq!(species_call(&rows), ("unresolved".to_owned(), None));
    assert_eq!(reason(&rows), "nearest_below_species_ANI");
}

#[test]
fn either_aligned_fraction_below_80_is_withheld() {
    for (aq, ar) in [(79.99, 90.0), (90.0, 79.99)] {
        let rows = [
            ("ref-a", Some("SP1"), 99.0, aq, ar),
            ("ref-b", Some("SP2"), 90.0, 90.0, 90.0),
        ];

        assert_eq!(reason(&rows), "species_coverage_not_met", "AF {aq}/{ar}");
    }
}

#[test]
fn a_qualifying_reference_other_than_the_nearest_can_witness_the_species() {
    let rows = [
        ("ref-a", Some("SP1"), 99.0, 50.0, 50.0),
        ("ref-b", Some("SP1"), 98.0, 85.0, 85.0),
        ("ref-c", Some("SP2"), 90.0, 90.0, 90.0),
    ];

    assert_eq!(
        species_call(&rows),
        ("assigned".to_owned(), Some("SP1".to_owned()))
    );
}

#[test]
fn a_tie_between_species_at_the_top_is_withheld() {
    let rows = [
        ("ref-a", Some("SP1"), 98.0, 90.0, 90.0),
        ("ref-b", Some("SP2"), 98.0, 90.0, 90.0),
    ];

    assert_eq!(reason(&rows), "different_species_nearest_tie");
}

#[test]
fn a_nearest_reference_without_a_species_is_withheld() {
    let rows = [
        ("ref-a", None, 98.0, 90.0, 90.0),
        ("ref-b", Some("SP2"), 90.0, 90.0, 90.0),
    ];

    assert_eq!(reason(&rows), "nearest_reference_has_no_species");
}

#[test]
fn a_panel_with_no_measured_competitor_is_withheld() {
    let rows = [
        ("ref-a", Some("SP1"), 99.0, 90.0, 90.0),
        ("ref-b", Some("SP1"), 98.0, 90.0, 90.0),
    ];

    assert_eq!(reason(&rows), "species_competitor_unmeasured");
}

#[test]
fn evidence_whose_counts_show_a_truncated_panel_is_rejected() {
    let mut evidence = evidence_json(&[
        ("ref-a", Some("SP1"), 99.0, 90.0, 90.0),
        ("ref-b", Some("SP2"), 90.0, 90.0, 90.0),
    ]);
    evidence["ranked_reference_evidence"]
        .as_array_mut()
        .unwrap()
        .pop();

    let error = SavedEvidence::from_json(&evidence.to_string()).unwrap_err();

    assert!(error.to_string().contains("truncated"), "{error}");
}

#[test]
fn evidence_with_more_than_two_decimals_is_rejected_not_rounded() {
    let rows = [
        ("ref-a", Some("SP1"), 95.999, 90.0, 90.0),
        ("ref-b", Some("SP2"), 90.0, 90.0, 90.0),
    ];

    assert!(SavedEvidence::from_json(&evidence_json(&rows).to_string()).is_err());
}
