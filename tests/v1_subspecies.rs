mod common;
use common::{decision_for, row};

#[test]
fn subspecies_needs_98_ani_80_both_af_and_a_one_point_margin() {
    let decision = decision_for(&[
        row("ref-a", Some("SP1"), 98.0, 80.0, 80.0).subspecies("SS1"),
        row("ref-b", Some("SP1"), 97.0, 10.0, 10.0).subspecies("SS2"),
        row("ref-c", Some("SP2"), 90.0, 90.0, 90.0),
    ]);
    assert_eq!(decision["subspecies"]["status"], "assigned");
    assert_eq!(decision["subspecies"]["id"], "SS1");
    assert_eq!(
        decision["assignment"]["subspecies_reason"],
        "diagnostic_candidate_only"
    );
}

#[test]
fn raw_low_af_competitor_still_prevents_a_subspecies_margin() {
    let decision = decision_for(&[
        row("ref-a", Some("SP1"), 99.0, 90.0, 90.0).subspecies("SS1"),
        row("ref-b", Some("SP1"), 98.01, 5.0, 5.0).subspecies("SS2"),
        row("ref-c", Some("SP2"), 90.0, 90.0, 90.0),
    ]);
    assert_eq!(decision["species"]["status"], "assigned");
    assert_eq!(decision["subspecies"]["id"], serde_json::Value::Null);
    assert_eq!(
        decision["subspecies"]["reason"],
        "global_group_margin_below_guide"
    );
}

#[test]
fn coverage_and_margin_must_be_satisfied_by_the_same_reference() {
    let decision = decision_for(&[
        row("ref-a", Some("SP1"), 99.5, 50.0, 50.0).subspecies("SS1"),
        row("ref-b", Some("SP1"), 98.99, 90.0, 90.0).subspecies("SS1"),
        row("ref-c", Some("SP1"), 98.0, 20.0, 20.0).subspecies("SS2"),
        row("ref-d", Some("SP2"), 90.0, 90.0, 90.0),
    ]);
    assert_eq!(
        decision["subspecies"]["reason"],
        "qualifying_reference_margin_below_guide"
    );
    assert_eq!(
        decision["rank_evidence"]["subspecies"]["ani_af_witness"]["reference_id"],
        "ref-b"
    );
}

#[test]
fn foreign_parent_competitor_is_included_in_the_margin() {
    let decision = decision_for(&[
        row("ref-a", Some("SP1"), 99.0, 90.0, 90.0).subspecies("SS1"),
        row("ref-b", Some("SP2"), 98.01, 10.0, 10.0),
        row("ref-c", Some("SP1"), 96.0, 10.0, 10.0).subspecies("SS2"),
    ]);
    assert_eq!(decision["species"]["status"], "assigned");
    assert_eq!(
        decision["subspecies"]["reason"],
        "global_group_margin_below_guide"
    );
    assert_eq!(
        decision["rank_evidence"]["subspecies"]["best_foreign_parent_competitor"]["ani_percent"],
        98.01
    );
}

#[test]
fn absence_of_a_within_parent_competitor_is_not_an_infinite_margin() {
    let decision = decision_for(&[
        row("ref-a", Some("SP1"), 99.0, 90.0, 90.0).subspecies("SS1"),
        row("ref-b", Some("SP2"), 90.0, 90.0, 90.0),
    ]);
    assert_eq!(
        decision["subspecies"]["reason"],
        "within_parent_competitor_unmeasured"
    );
}

#[test]
fn within_parent_ties_and_species_only_references_withhold_subspecies() {
    for alternative in [
        row("ref-b", Some("SP1"), 99.0, 10.0, 10.0).subspecies("SS2"),
        row("ref-b", Some("SP1"), 99.0, 10.0, 10.0),
    ] {
        let decision = decision_for(&[
            row("ref-a", Some("SP1"), 99.0, 90.0, 90.0).subspecies("SS1"),
            alternative,
            row("ref-c", Some("SP2"), 90.0, 90.0, 90.0),
        ]);
        assert_eq!(
            decision["subspecies"]["reason"],
            "within_parent_nearest_group_tie"
        );
    }
}

#[test]
fn exact_subspecies_ani_and_af_boundaries_are_inclusive() {
    for (ani, aq, ar, reason) in [
        (97.99, 90.0, 90.0, "nearest_group_below_subspecies_ANI"),
        (98.0, 79.99, 90.0, "subspecies_coverage_not_met"),
        (98.0, 90.0, 79.99, "subspecies_coverage_not_met"),
        (98.0, 80.0, 80.0, "diagnostic_candidate_only"),
    ] {
        let decision = decision_for(&[
            row("ref-a", Some("SP1"), ani, aq, ar).subspecies("SS1"),
            row("ref-b", Some("SP1"), 97.5, 90.0, 90.0).subspecies("SS1"),
            row("ref-c", Some("SP1"), 96.9, 10.0, 10.0).subspecies("SS2"),
            row("ref-d", Some("SP2"), 90.0, 90.0, 90.0),
        ]);
        assert_eq!(decision["subspecies"]["reason"], reason);
    }
}
