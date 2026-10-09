//! Saved ANIm is accepted only when bound to this query, the expected method profile, the
//! panel's reference bytes and, under the dry-plan rule, the pairs the plan requested.
mod common;

use common::p1::{self, hit, reciprocal, PROFILE, QUERY_SHA256};
use serde_json::{json, Value};
use terobacter::evidence::SavedEvidence;
use terobacter::panel::Panel;
use terobacter::rescue::{AnimRecords, BoundAnim, RescueInput, Thresholds};

fn excluded(decision: &Value) -> Vec<(String, String)> {
    decision["input"]["anim"]["excluded"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["reference_id"].as_str().unwrap().into(),
                e["reason"].as_str().unwrap().into(),
            )
        })
        .collect()
}

fn planned(case: &Value) -> Value {
    let (rescue, records) = p1::input("binding", &case["input"], &[]).unwrap();
    let anim = rescue.bind_to_plan(&records, PROFILE).unwrap();
    rescue.decide(&anim).unwrap().to_value().clone()
}

fn records_with(case: &Value, edit: impl Fn(&str) -> String) -> AnimRecords {
    AnimRecords::from_jsonl(&edit(&p1::records_text(&case["input"]))).unwrap()
}

#[test]
fn records_for_another_query_are_skipped_and_counted() {
    let case = p1::case("strict_above_promotes");
    let (rescue, _) = p1::input("binding", &case["input"], &[]).unwrap();
    let other = "c".repeat(64);
    let records = records_with(&case, |t| t.replace(QUERY_SHA256, &other));
    let anim = rescue.bind_supplied(&records, PROFILE).unwrap();
    let d = rescue.decide(&anim).unwrap();
    let d = d.to_value();
    assert_eq!(d["input"]["anim"]["other_query_records"], 2);
    assert_eq!(d["input"]["anim"]["bound_pairs"], json!([]));
    assert_eq!(d["reasons"], json!(["ANIm_incomplete"]));
    assert_eq!(d["rescue_status"], "incomplete");
}

#[test]
fn a_record_for_this_query_with_stale_bindings_is_an_error() {
    let case = p1::case("strict_above_promotes");
    let (rescue, _) = p1::input("binding", &case["input"], &[]).unwrap();
    let stale_reference = records_with(&case, |t| t.replace(&p1::sha256("A1"), &"f".repeat(64)));
    let other_length = records_with(&case, |t| {
        t.replacen(
            "\"total_bases\":1000000000000",
            "\"total_bases\":2000000000000",
            1,
        )
        .replacen(
            "\"query_length\":1000000000000",
            "\"query_length\":2000000000000",
            2,
        )
    });
    for (records, expected) in [(stale_reference, "hash"), (other_length, "query length")] {
        let error = rescue.bind_supplied(&records, PROFILE).unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
    let records = records_with(&case, |t| t.to_owned());
    let error = rescue
        .bind_supplied(&records, "another-profile")
        .unwrap_err();
    assert!(error.to_string().contains("profile"), "{error}");
}

#[test]
fn identical_duplicate_records_are_one_pair_and_conflicting_ones_are_rejected() {
    let case = p1::case("strict_above_promotes");
    let (rescue, _) = p1::input("binding", &case["input"], &[]).unwrap();
    let text = p1::records_text(&case["input"]);
    let first = text.lines().next().unwrap();
    let same = AnimRecords::from_jsonl(&format!("{text}\n{first}")).unwrap();
    let d = rescue
        .decide(&rescue.bind_supplied(&same, PROFILE).unwrap())
        .unwrap();
    assert_eq!(d.to_value()["species_id"], "A");
    let changed = first.replacen("\"E\":", "\"E\":1", 1);
    let conflicting = AnimRecords::from_jsonl(&format!("{text}\n{changed}")).unwrap();
    let error = rescue.bind_supplied(&conflicting, PROFILE).unwrap_err();
    assert!(error.to_string().contains("conflicting"), "{error}");
}

#[test]
fn the_dry_plan_rule_excludes_anim_for_an_out_of_band_record() {
    let case = p1::case("skani_96plus_keeps_supplied_ANIm_veto");
    // As supplied evidence, P1 keeps the old veto outside the band ...
    assert_eq!(
        p1::decide(&case).unwrap()["reasons"],
        json!(["method_gate_disagreement"])
    );
    // ... but the dry plan requests no pair for this record, so its ANIm is not bound.
    let d = planned(&case);
    assert_eq!(d["status"], "assigned");
    assert_eq!(d["call_basis"], "skani");
    assert_eq!(d["reasons"], json!([]));
    assert_eq!(
        excluded(&d),
        [
            ("A1".into(), "not_requested_by_dry_plan".into()),
            ("B1".into(), "not_requested_by_dry_plan".into())
        ]
    );
    assert_eq!(d["legacy_v1_result"]["status"], d["status"]);
}

#[test]
fn competitor_records_are_bound_only_once_a_witness_passes() {
    // The witness fails the strict ANIm gate, so the plan never requests B1.
    let d = planned(&p1::case("ANIm_below96"));
    assert_eq!(d["reasons"], json!(["ANIm_no_strict_witness"]));
    assert_eq!(
        excluded(&d),
        [("B1".into(), "not_requested_by_dry_plan".into())]
    );
    assert_eq!(d["input"]["anim"]["bound_pairs"], json!(["A1"]));

    // A passing witness requests the whole panel, so B1 is bound and the rescue completes.
    let d = planned(&p1::case("strict_above_promotes"));
    assert_eq!(d["species_id"], "A");
    assert_eq!(excluded(&d), []);
    assert_eq!(d["input"]["anim"]["binding"], "dry_plan_requested_pairs");
}

#[test]
fn a_record_blocked_before_anim_binds_no_pairs() {
    let d = planned(&p1::case("warnings_veto"));
    assert_eq!(d["competitor_evaluation"], "not_run_already_blocked");
    assert_eq!(d["reasons"], json!(["external_problem_warning"]));
    assert_eq!(excluded(&d).len(), 2);
}

#[test]
fn bindings_follow_the_mask_setting() {
    // The same records, under a setting that masks the witness: A2 becomes the only candidate,
    // so A1's record is masked and B1's is not requested until A2 has a passing record.
    let input = json!({
        "references": [hit("A1", Some("A"), "95.5", "85", "85"), hit("A2", Some("A"), "95", "85", "85"),
            hit("B1", Some("B"), "90", "85", "85")],
        "anim": {"A1": reciprocal("96.2", "85", "85"), "B1": reciprocal("90", "85", "85")}});
    let (rescue, records) = p1::input("binding", &input, &["A1"]).unwrap();
    let d = rescue
        .decide(&rescue.bind_to_plan(&records, PROFILE).unwrap())
        .unwrap();
    let d = d.to_value();
    assert_eq!(
        excluded(d),
        [
            ("A1".into(), "masked_reference".into()),
            ("B1".into(), "not_requested_by_dry_plan".into())
        ]
    );
    assert_eq!(d["reasons"], json!(["ANIm_incomplete"]));
    assert_eq!(d["rescue_status"], "incomplete");
}

#[test]
fn records_with_unknown_states_swapped_roles_or_another_scope_are_rejected() {
    let case = p1::case("strict_above_promotes");
    let text = p1::records_text(&case["input"]);
    let first = text.lines().next().unwrap();
    assert!(AnimRecords::from_jsonl(first).is_ok());
    for (from, to, expected) in [
        (
            "\"native_reference_id\":\"A1\",\"status\":\"aligned\"}",
            "\"native_reference_id\":\"A1\",\"status\":\"alligned\"}",
            "unknown ANIm orientation status",
        ),
        (
            "\"native_query_id\":\"query\"",
            "\"native_query_id\":\"A1\"",
            "native reference and query",
        ),
        (
            "\"scope\":\"whole_assembly\"",
            "\"scope\":\"chromosome\"",
            "binding",
        ),
        (
            "\"component\":\"primary_whole\"",
            "\"component\":\"historical_chromosomes\"",
            "component",
        ),
    ] {
        let changed = first.replacen(from, to, 1);
        assert_ne!(changed, first, "{from}");
        let error = AnimRecords::from_jsonl(&changed).unwrap_err();
        assert!(error.to_string().contains(expected), "{expected}: {error}");
    }
}

/// Without a query ID the reciprocal roles cannot be checked, so a query binding needs one,
/// even when the native IDs would otherwise contradict their direction.
#[test]
fn a_query_binding_without_a_query_id_is_rejected() {
    let case = p1::case("strict_above_promotes");
    let text = p1::records_text(&case["input"]);
    let first = text.lines().next().unwrap();
    let swapped = first.replace(
        "\"native_query_id\":\"query\"",
        "\"native_query_id\":\"A1\"",
    );
    for changed in [
        first.replacen("\"id\":\"query\",", "", 1),
        first.replacen("\"id\":\"query\"", "\"id\":\" \"", 1),
        swapped.replacen("\"id\":\"query\",", "", 1),
    ] {
        assert_ne!(changed, first);
        let error = AnimRecords::from_jsonl(&changed).unwrap_err();
        assert!(
            error.to_string().contains("ANIm query ID required"),
            "{error}"
        );
    }
}

#[test]
fn a_single_pretty_printed_pair_record_is_read() {
    let case = p1::case("strict_above_promotes");
    let text = p1::records_text(&case["input"]);
    let first: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    let pretty = serde_json::to_string_pretty(&first).unwrap();
    assert!(AnimRecords::from_jsonl(&pretty).is_err());
    assert_eq!(AnimRecords::from_json(&pretty).unwrap().len(), 1);
}

/// Bound ANIm belongs to the input it was bound under. Another query, panel, mask, threshold
/// setting or warning set refuses it; empty ANIm is accepted everywhere.
#[test]
fn anim_bound_for_one_input_is_refused_by_another() {
    let input = json!({"references": [hit("A1", Some("A"), "95.5", "85", "85"),
            hit("B1", Some("B"), "90", "85", "85")],
        "anim": {"A1": reciprocal("96.2", "85", "85"), "B1": reciprocal("90", "85", "85")}});
    let (rescue, records) = p1::input("bound", &input, &[]).unwrap();
    let bound = rescue.bind_supplied(&records, PROFILE).unwrap();
    assert_eq!(
        rescue.decide(&bound).unwrap().to_value()["status"],
        "assigned"
    );

    // Query B: different skani evidence, and no ANIm of its own for A1.
    let mut query_b = input.clone();
    query_b["references"][0] = hit("A1", Some("A"), "95.4", "85", "85");
    query_b["anim"] = json!({"B1": reciprocal("90", "85", "85")});
    let (other_query, other_records) = p1::input("bound", &query_b, &[]).unwrap();
    // The same panel bytes but another inventory file.
    let evidence = SavedEvidence::from_json(&p1::evidence_text("bound", &input)).unwrap();
    let panel = Panel::from_json(&format!("{}\n", p1::inventory_text(&input)), &evidence).unwrap();
    let other_panel = RescueInput::new(evidence, panel, &[], Thresholds::standard()).unwrap();
    let (masked, _) = p1::input("bound", &input, &["B1"]).unwrap();
    let mut custom = input.clone();
    custom["custom"] = json!({"species_ani": "96"});
    let (custom, _) = p1::input("bound", &custom, &[]).unwrap();
    let mut warned = input.clone();
    warned["warnings"] = json!(["bound_problem"]);
    let (warned, _) = p1::input("bound", &warned, &[]).unwrap();

    for (name, other) in [
        ("query", &other_query),
        ("panel", &other_panel),
        ("mask", &masked),
        ("thresholds", &custom),
        ("warnings", &warned),
    ] {
        let refused = other.decide(&bound).unwrap_err().to_string();
        assert!(
            refused.contains("bound to a different"),
            "{name}: {refused}"
        );
        assert!(other.plan(&bound).is_err(), "{name}");
        assert!(other.decide(&BoundAnim::none()).is_ok(), "{name}");
    }
    // Rebound to query B, the same records leave its witness unmeasured.
    let rebound = other_query.bind_supplied(&other_records, PROFILE).unwrap();
    let d = other_query.decide(&rebound).unwrap();
    assert_eq!(d.to_value()["status"], "unresolved");
    assert_eq!(d.to_value()["reasons"], json!(["ANIm_incomplete"]));
}
