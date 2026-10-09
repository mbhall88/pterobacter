mod common;
use common::{decision_for, row};
use serde_json::{json, Value};
#[test]
fn sub_one_percent_af_keeps_its_declared_units() {
    let d = decision_for(&[
        row("ref-a", Some("SP1"), 99.0, 0.8, 0.8),
        row("ref-b", Some("SP2"), 90.0, 90.0, 90.0),
    ]);
    assert_eq!(d["assignment"]["status"], "unresolved");
    assert_eq!(d["assignment"]["reason"], "species_coverage_not_met");
    for k in [
        "species_id",
        "species_name",
        "subspecies_id",
        "subspecies_name",
        "lineage_id",
        "genomovar_id",
    ] {
        assert_eq!(d["assignment"][k], Value::Null);
    }
    assert_eq!(
        d["rank_evidence"]["species"]["raw_nearest"]["tied_references"][0]
            ["aligned_fraction_query_percent"],
        0.8
    );
}
#[test]
fn near_boundary_is_report_only_with_the_frozen_band_and_tied_witness() {
    let d = decision_for(&[
        row("ref-a", Some("SP1"), 95.9, 70.0, 90.0),
        row("ref-b", Some("SP1"), 95.9, 80.0, 80.0),
        row("ref-c", Some("SP2"), 90.0, 90.0, 90.0),
    ]);
    assert_eq!(d["assignment"]["status"], "unresolved");
    assert_eq!(d["near_species_boundary"]["reference_id"], "ref-b");
    assert_eq!(d["near_species_boundary"]["is_call"], false);
    assert_eq!(
        d["near_species_boundary"]["band"],
        json!({"ani_minimum":"95","both_af_minimum":"80","species_call_ani":"96"})
    );
    assert_eq!(
        d["near_species_boundary"]["message"],
        "closest species: Enterobacter SP1 at 95.9% ANI, below the 96% call threshold; not a call"
    );
}
#[test]
fn near_boundary_needs_unique_named_parent_and_raw_nearest_coverage() {
    for rows in [
        vec![
            row("ref-a", Some("SP1"), 95.9, 70.0, 90.0),
            row("ref-b", Some("SP1"), 95.8, 90.0, 90.0),
            row("ref-c", Some("SP2"), 90.0, 90.0, 90.0),
        ],
        vec![
            row("ref-a", Some("SP1"), 95.9, 90.0, 90.0),
            row("ref-b", Some("SP2"), 95.9, 90.0, 90.0),
        ],
        vec![
            row("ref-a", None, 95.9, 90.0, 90.0),
            row("ref-b", Some("SP2"), 90.0, 90.0, 90.0),
        ],
    ] {
        assert_eq!(decision_for(&rows)["near_species_boundary"], Value::Null);
    }
}
