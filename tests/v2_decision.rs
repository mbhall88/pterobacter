//! v2 decisions beyond the P1 fixture file: required report fields (R7), incomplete versus
//! negative outcomes, the v1 comparator, warning combinations and competitors outside the top
//! five.
mod common;

use common::p1::{self, hit, reciprocal, statuses, unreturned};
use serde_json::{json, Value};
use terobacter::evidence::SavedEvidence;

fn decide(input: &Value) -> Value {
    p1::decide(&json!({"id": "decision", "input": input})).unwrap()
}

/// A rescue of species A at skani 95.5 with every competitor measured.
fn rescuable() -> Value {
    json!({"references": [hit("A1", Some("A"), "95.5", "85", "85"), hit("B1", Some("B"), "90", "85", "85"),
            unreturned("C1", Some("C"))],
        "anim": {"A1": reciprocal("96.2", "85", "85"), "B1": reciprocal("90", "85", "85"),
            "C1": reciprocal("88", "85", "85")}})
}

#[test]
fn every_v2_result_carries_the_p1_decision_completeness_count_and_comparator_fields() {
    for case in p1::cases().iter().filter(|c| c.get("raises").is_none()) {
        let id = case["id"].as_str().unwrap();
        let d = p1::decide(case).unwrap();
        assert_eq!(d["schema"], "3.0.0-conditional-assembly-report", "{id}");
        assert_eq!(d["policy_id"], "conservative-assembly-anim-v2", "{id}");
        assert_eq!(d["contract_revision"], "P1-r1", "{id}");
        let custom = case["input"].get("custom").is_some();
        assert_eq!(
            d["validation_status"],
            if custom {
                "custom_unvalidated"
            } else {
                "not_yet_validated"
            },
            "{id}"
        );
        for key in [
            "trigger",
            "trigger_state",
            "call_basis",
            "reasons",
            "competitor_evaluation",
            "rescue_status",
            "evaluation_status",
            "status",
            "species_id",
            "subspecies_id",
            "subspecies_reason",
            "challenges",
            "top_references",
            "panel_summary",
        ] {
            assert!(d.get(key).is_some(), "{id}: {key}");
        }
        for key in [
            "required",
            "attempted",
            "completed",
            "no_alignment",
            "failed",
            "missing_orientation",
            "not_run",
        ] {
            assert!(d["counts"][key].is_u64(), "{id}: counts.{key}");
        }
        for key in [
            "policy_id",
            "status",
            "species_id",
            "subspecies_id",
            "reasons",
        ] {
            assert!(
                d["legacy_v1_result"].get(key).is_some(),
                "{id}: legacy_v1_result.{key}"
            );
        }
        assert_eq!(
            d["legacy_v1_result"]["policy_id"], "conservative-assembly-v1",
            "{id}"
        );
    }
}

#[test]
fn a_rescue_changes_species_only_and_reports_v1_beside_it() {
    let mut input = rescuable();
    input["anim"]["A1"] = json!({"directions": [
        {"id": "QR", "status": "complete", "ani": "96.4", "aq": "85", "ar": "86"},
        {"id": "RQ", "status": "complete", "ani": "96.2", "aq": "81", "ar": "85"}]});
    let d = decide(&input);
    assert_eq!(d["status"], "assigned");
    assert_eq!(d["species_id"], "A");
    assert_eq!(d["species_name"], "Synthetic A");
    assert_eq!(d["subspecies_id"], Value::Null);
    assert_eq!(
        d["subspecies_reason"],
        "ANIm_species_rescue_does_not_establish_subspecies"
    );
    assert_eq!(d["rescue_status"], "rescued");
    assert_eq!(d["witness"]["reference_id"], "A1");
    assert_eq!(d["witness"]["anim"]["QR"]["D"], 2_000_000_000_000u64);
    // Conservative minima across both orientations, never an average.
    assert_eq!(d["witness"]["anim"]["minimum_identity_percent"], 96.2);
    assert_eq!(d["witness"]["anim"]["minimum_af_percent"], 81.0);
    assert_eq!(d["legacy_v1_result"]["status"], "unresolved");
    assert_eq!(d["legacy_v1_result"]["species_id"], Value::Null);
    assert_eq!(
        d["counts"],
        json!({"unit": "reciprocal_pair", "required": 3, "attempted": 3,
        "completed": 3, "no_alignment": 0, "failed": 0, "missing_orientation": 0, "not_run": 0,
        "supplied_outside_plan": 0})
    );
}

#[test]
fn missing_or_failed_measurements_are_incomplete_never_negative() {
    let failed_witness = {
        let mut i = rescuable();
        i["anim"]["A1"] = statuses("error", "error");
        i
    };
    let missing_competitor = {
        let mut i = rescuable();
        i["anim"].as_object_mut().unwrap().remove("C1");
        i
    };
    let failed_competitor = {
        let mut i = rescuable();
        i["anim"]["C1"] = statuses("complete", "error");
        i
    };
    for (input, reason, failed, not_run) in [
        (failed_witness, "ANIm_incomplete", 1, 0),
        (missing_competitor, "ANIm_competitor_incomplete", 0, 1),
        (failed_competitor, "ANIm_competitor_incomplete", 1, 0),
    ] {
        let d = decide(&input);
        assert_eq!(d["status"], "unresolved", "{reason}");
        assert!(
            d["reasons"].as_array().unwrap().contains(&json!(reason)),
            "{d}"
        );
        assert!(!d["reasons"]
            .as_array()
            .unwrap()
            .contains(&json!("ANIm_no_strict_witness")));
        assert_eq!(d["rescue_status"], "incomplete");
        assert_eq!(d["evaluation_status"], "incomplete");
        assert_eq!(d["counts"]["failed"], failed, "{reason}");
        assert_eq!(d["counts"]["not_run"], not_run, "{reason}");
    }
}

#[test]
fn a_one_sided_challenge_with_a_failed_other_orientation_keeps_both_reasons() {
    let mut input = rescuable();
    input["anim"]["C1"] = json!({"directions": [
        {"id": "QR", "status": "complete", "ani": "97", "aq": "20", "ar": "85"},
        {"id": "RQ", "status": "error"}]});
    let d = decide(&input);
    assert_eq!(
        d["reasons"],
        json!(["ANIm_competitor_incomplete", "foreign_parent_challenge"])
    );
    assert_eq!(d["rescue_status"], "withheld");
    assert_eq!(d["evaluation_status"], "incomplete");
    assert_eq!(
        d["challenge_evidence"][0]["anim_challenging_orientations"],
        json!(["QR"])
    );
}

#[test]
fn a_challenge_outside_the_top_five_is_shown_in_the_panel_summary() {
    let p = p1::case("competitor_outside_topfive_cannot_disappear");
    let d = p1::decide(&p).unwrap();
    let b = d["panel_summary"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["parent"]["concept_id"] == "B")
        .unwrap()
        .clone();
    assert_eq!(b["in_top_references"], false);
    assert_eq!(b["challenging_reference_ids"], json!(["B1"]));
    assert!(d["reasons"]
        .as_array()
        .unwrap()
        .contains(&json!("foreign_parent_challenge")));
}

#[test]
fn exact_ties_get_one_display_group_and_never_select_a_winner() {
    let d = p1::decide(&p1::case("display_fifth_tie_included")).unwrap();
    let groups: Vec<_> = d["top_references"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["reference_id"].as_str().unwrap().to_owned(),
                r["tie_group"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(groups[4..], [("A5".to_owned(), 4), ("A6".to_owned(), 4)]);
    let tie = p1::decide(&p1::case("cross_species_skani_tie_stays_unresolved")).unwrap();
    assert_eq!(tie["species_id"], Value::Null);
    assert_eq!(tie["nearest_parent"]["kind"], "species");
    assert_eq!(tie["trigger_state"], "triggered_cross_parent_tie");
}

#[test]
fn simultaneous_warnings_and_measurement_failures_are_all_retained() {
    let mut input = rescuable();
    input["warnings"] = json!(["problem-1", "problem-2"]);
    input["scope"] = json!("unexplained_after_checks");
    input["anim"]["A1"] = statuses("error", "complete");
    input["anim"]["C1"] = reciprocal("96.5", "85", "85");
    let d = decide(&input);
    assert_eq!(
        d["reasons"],
        json!([
            "ANIm_incomplete",
            "external_problem_warning",
            "unexplained_residual_sequence"
        ])
    );
    assert_eq!(d["competitor_evaluation"], "not_run_already_blocked");
    assert_eq!(d["rescue_status"], "withheld");
    // A known challenge stays visible even though a veto settled the result first.
    assert_eq!(d["challenges"], json!(["C1"]));
    assert_eq!(d["external_problem_warnings"].as_array().unwrap().len(), 2);
    assert_eq!(
        d["legacy_v1_result"]["reasons"],
        json!([
            "nearest_below_species_ANI",
            "unexplained_residual_sequence",
            "external_problem_warning"
        ])
    );
}

#[test]
fn outside_the_band_without_anim_v2_reproduces_the_v1_comparator() {
    for id in [
        "skani_exact96_uses_legacy_route",
        "ordinary_subspecies_margin1_inclusive",
        "ordinary_subspecies_below_margin",
        "two_skani_parents_alert_low_queryAF",
        "below_trigger_evidence_only",
        "no_skani_hits_no_rescue",
    ] {
        let mut case = p1::case(id);
        case["input"]["anim"] = json!({});
        let d = p1::decide(&case).unwrap();
        let v1 = &d["legacy_v1_result"];
        assert_eq!(d["trigger"], false, "{id}");
        assert_eq!(d["rescue_status"], "not_triggered", "{id}");
        for key in ["status", "species_id", "subspecies_id", "subspecies_reason"] {
            assert_eq!(d[key], v1[key], "{id}: {key}");
        }
        let sorted = |v: &Value| {
            let mut r: Vec<_> = v
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r.to_string())
                .collect();
            r.sort();
            r
        };
        assert_eq!(sorted(&d["reasons"]), sorted(&v1["reasons"]), "{id}");
    }
}

#[test]
fn skani_values_with_more_than_two_decimals_are_rejected_not_rounded_into_the_band() {
    let input = json!({"references": [hit("A1", Some("A"), "94.49", "85", "85"),
        hit("B1", Some("B"), "90", "85", "85")]});
    let text = p1::evidence_text("direct", &input);
    assert!(SavedEvidence::from_json(&text).is_ok());
    let literal = text.replace("94.49", "94.49999");
    assert_ne!(literal, text);
    assert!(SavedEvidence::from_json(&literal).is_err());
}

#[test]
fn custom_thresholds_equal_to_the_defaults_are_still_custom_and_recorded() {
    let mut input = rescuable();
    input["custom"] = json!({"species_ani": "96.0", "subspecies_ani": 98});
    let d = decide(&input);
    assert_eq!(d["species_id"], "A");
    assert_eq!(d["policy_mode"], "custom");
    assert_eq!(d["validation_status"], "custom_unvalidated");
    assert_eq!(
        d["thresholds"]["supplied"],
        json!({"species_ani": "96.0", "subspecies_ani": "98"})
    );
    assert_eq!(d["thresholds"]["species_ani"], "96");
    assert_eq!(d["thresholds"]["trigger_lower_ani"], "94.5");
    let standard = decide(&rescuable());
    assert_eq!(standard["policy_mode"], "default_candidate");
    assert_eq!(standard["thresholds"]["supplied"], Value::Null);
}

#[test]
fn every_v2_reason_maps_to_the_contract_vocabulary() {
    let map: Vec<(String, String)> = common::reason_vocabulary("conservative-assembly-anim-v2")
        .into_iter()
        .map(|row| (row.terobacter_reason, row.field))
        .collect();
    let known = |reason: &str, field: &str| map.iter().any(|(r, f)| r == reason && f == field);
    let mut extra = vec![decide(&rescuable())];
    let mut failed = rescuable();
    failed["anim"]["A1"] = statuses("error", "error");
    extra.push(decide(&failed));
    let decisions = p1::cases()
        .into_iter()
        .filter(|c| c.get("raises").is_none())
        .map(|c| p1::decide(&c).unwrap())
        .chain(extra);
    for d in decisions {
        for reason in d["reasons"].as_array().unwrap() {
            assert!(
                known(reason.as_str().unwrap(), "reasons"),
                "unmapped reason {reason}"
            );
        }
        let sub = d["subspecies_reason"].as_str().unwrap();
        assert!(
            known(sub, "subspecies_reason"),
            "unmapped subspecies reason {sub}"
        );
    }
}
