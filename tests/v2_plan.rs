//! The dry rescue plan (R4): trigger, nearest parent, witnesses and outstanding competitors,
//! from saved evidence and optionally witness ANIm. It never runs an alignment.
mod common;

use common::p1::{self, hit, reciprocal, statuses, unreturned, PROFILE};
use serde_json::{json, Value};
use terobacter::rescue::{AnimRecords, BoundAnim};

fn plan(input: &Value, anim: &[(&str, Value)]) -> Value {
    let mut input = input.clone();
    input["anim"] = json!({});
    for (id, pair) in anim {
        input["anim"][*id] = pair.clone();
    }
    let (rescue, records) = p1::input("plan", &input, &[]).unwrap();
    let bound = rescue.bind_to_plan(&records, PROFILE).unwrap();
    rescue.plan(&bound).to_value().clone()
}

/// Species A nearest at 95.5 (A2), two more A candidates, an A reference with low skani AF, a
/// B contrast, an unnamed lineage and a reference skani did not return. Witness order (A2, A1,
/// A3) differs from ID order, and the non-witness A0 sorts before every candidate.
fn triggered() -> Value {
    json!({"references": [
        hit("A1", Some("A"), "95", "85", "85"), hit("A2", Some("A"), "95.5", "85", "85"),
        hit("A3", Some("A"), "94.5", "80", "80"), hit("A0", Some("A"), "95", "79.99", "85"),
        hit("B1", Some("B"), "93", "85", "85"),
        json!({"id": "L1", "species": null, "subspecies": null, "lineage": "LIN1", "ani": "92", "aq": "85", "ar": "85"}),
        unreturned("C1", Some("C"))]})
}

fn ids(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            v.as_str()
                .map_or_else(|| v["reference_id"].as_str().unwrap().into(), str::to_owned)
        })
        .collect()
}

#[test]
fn a_first_plan_requests_the_skani_witnesses_in_deterministic_order() {
    let p = plan(&triggered(), &[]);
    assert_eq!(p["anim_executed"], false);
    assert_eq!(p["trigger"], true);
    assert_eq!(p["trigger_state"], "triggered_with_witness_candidate");
    assert_eq!(p["max_skani_ani_percent"], 95.5);
    assert_eq!(p["nearest_parent"]["kind"], "species");
    assert_eq!(p["nearest_parent"]["concept_id"], "A");
    assert_eq!(p["stage"], "witness");
    // Decreasing skani ANI, then reference ID; A0 fails a skani AF and cannot witness.
    assert_eq!(ids(&p["witness_candidates"]), ["A2", "A1", "A3"]);
    assert_eq!(ids(&p["requested_pairs"]), ["A2", "A1", "A3"]);
    assert_eq!(ids(&p["outstanding_witnesses"]), ["A2", "A1", "A3"]);
    assert_eq!(p["outstanding_competitors"], json!([]));
    assert!(p["requested_pairs"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["stage"] == "witness" && r["status"] == "outstanding"));
    assert_eq!(
        p["requested_pairs"][0]["reference_sha256"],
        p1::sha256("A2")
    );
    assert_eq!(p["counts"]["required"], 3);
    assert_eq!(p["counts"]["not_run"], 3);
    assert_eq!(p["validation_status"], "not_yet_validated");
}

#[test]
fn a_passing_witness_requests_every_remaining_panel_reference() {
    let p = plan(
        &triggered(),
        &[
            ("A1", statuses("error", "complete")),
            ("A2", reciprocal("96.1", "80", "80")),
        ],
    );
    assert_eq!(p["stage"], "competitor_panel");
    assert_eq!(p["passing_witness"], "A2");
    // Unnamed, unreturned, low-AF same-species and the failed witness are all still required:
    // the remaining witness candidates first, in witness order, then the rest of the panel by ID.
    assert_eq!(
        ids(&p["outstanding_competitors"]),
        ["A1", "A3", "A0", "B1", "C1", "L1"]
    );
    let statuses: Vec<_> = p["requested_pairs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["reference_id"].as_str().unwrap(),
                r["stage"].as_str().unwrap(),
                r["status"].as_str().unwrap(),
            )
        })
        .collect();
    assert!(statuses.contains(&("A1", "witness", "failed")));
    assert!(statuses.contains(&("A2", "witness", "measured")));
    assert!(statuses.contains(&("C1", "competitor_panel", "outstanding")));
    assert_eq!(p["counts"]["required"], 7);
    assert_eq!(p["counts"]["attempted"], 2);
    assert_eq!(p["counts"]["completed"], 1);
    assert_eq!(p["counts"]["failed"], 1);
    assert_eq!(p["counts"]["not_run"], 5);
}

#[test]
fn a_known_foreign_challenge_is_reported_as_a_decisive_veto() {
    let p = plan(
        &triggered(),
        &[
            ("A1", reciprocal("96.1", "85", "85")),
            ("C1", reciprocal("96", "10", "80")),
        ],
    );
    assert_eq!(p["known_challenges"], json!(["C1"]));
    assert_eq!(p["decisive_vetoes"], json!(["foreign_parent_challenge"]));
    assert!(ids(&p["outstanding_competitors"]).contains(&"B1".to_owned()));
}

#[test]
fn a_failed_witness_leaves_the_plan_at_the_witness_stage() {
    let p = plan(&triggered(), &[("A2", statuses("error", "error"))]);
    assert_eq!(p["stage"], "witness");
    assert_eq!(p["passing_witness"], Value::Null);
    assert_eq!(ids(&p["outstanding_witnesses"]), ["A2", "A1", "A3"]);
}

#[test]
fn every_untriggered_or_blocked_state_requests_no_pairs() {
    let b = hit("B1", Some("B"), "90", "85", "85");
    let cases = [
        (
            vec![unreturned("A1", Some("A")), unreturned("B1", Some("B"))],
            "no_estimate",
            false,
        ),
        (
            vec![hit("A1", Some("A"), "96", "85", "85"), b.clone()],
            "at_or_above_96_v1_route",
            false,
        ),
        (
            vec![hit("A1", Some("A"), "94.49", "85", "85"), b.clone()],
            "below_band_v1_route",
            false,
        ),
        (
            vec![
                hit("A1", Some("A"), "95", "85", "85"),
                hit("B0", Some("B"), "95", "85", "85"),
            ],
            "triggered_cross_parent_tie",
            true,
        ),
        (
            vec![
                json!({"id": "A1", "species": null, "subspecies": null, "lineage": "LIN", "ani": "95", "aq": "85", "ar": "85"}),
                b.clone(),
            ],
            "triggered_unnamed_nearest",
            true,
        ),
        (
            vec![
                hit("A1", Some("A"), "95", "85", "85"),
                unreturned("B1", Some("B")),
            ],
            "triggered_no_measured_contrast",
            true,
        ),
        (
            vec![hit("A1", Some("A"), "95", "79", "85"), b.clone()],
            "triggered_no_skani_witness",
            true,
        ),
    ];
    for (references, state, trigger) in cases {
        let p = plan(&json!({"references": references}), &[]);
        assert_eq!(p["trigger_state"], state);
        assert_eq!(p["trigger"], trigger, "{state}");
        assert_eq!(p["requested_pairs"], json!([]), "{state}");
        assert_eq!(p["counts"]["required"], 0, "{state}");
        if trigger {
            assert!(
                !p["blocking_reasons"].as_array().unwrap().is_empty(),
                "{state}"
            );
        }
    }
}

#[test]
fn a_hard_veto_blocks_the_plan_but_keeps_the_witness_candidates_visible() {
    let mut input = triggered();
    input["warnings"] = json!(["bound_problem"]);
    let p = plan(&input, &[]);
    assert_eq!(p["trigger_state"], "triggered_with_witness_candidate");
    assert_eq!(p["stage"], "blocked_before_anim");
    assert_eq!(p["blocking_reasons"], json!(["external_problem_warning"]));
    assert_eq!(ids(&p["witness_candidates"]), ["A2", "A1", "A3"]);
    assert_eq!(p["requested_pairs"], json!([]));
}

#[test]
fn the_plan_does_not_depend_on_record_order() {
    let input = json!({"references": triggered()["references"],
        "anim": {"A1": reciprocal("96.1", "85", "85"), "B1": reciprocal("90", "85", "85")}});
    let (rescue, _) = p1::input("plan", &input, &[]).unwrap();
    let text = p1::records_text(&input);
    let reversed: Vec<_> = text.lines().rev().collect();
    let plan_of = |t: &str| {
        let records = AnimRecords::from_jsonl(t).unwrap();
        rescue
            .plan(&rescue.bind_to_plan(&records, PROFILE).unwrap())
            .to_json()
    };
    assert_eq!(plan_of(&text), plan_of(&reversed.join("\n")));
    assert_ne!(plan_of(&text), rescue.plan(&BoundAnim::none()).to_json());
}

#[test]
fn a_pair_with_one_orientation_is_counted_apart_from_a_failed_one() {
    let one_way = json!({"directions": [{"id": "QR", "status": "complete", "ani": "97", "aq": "85", "ar": "85"}]});
    let p = plan(
        &triggered(),
        &[("A2", one_way), ("A1", statuses("complete", "error"))],
    );
    assert_eq!(p["stage"], "witness");
    let status = |id: &str| {
        p["requested_pairs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["reference_id"] == id)
            .unwrap()["status"]
            .clone()
    };
    assert_eq!(status("A2"), "missing_orientation");
    assert_eq!(status("A1"), "failed");
    assert_eq!(p["counts"]["attempted"], 2);
    assert_eq!(p["counts"]["completed"], 0);
    assert_eq!(p["counts"]["failed"], 1);
    assert_eq!(p["counts"]["missing_orientation"], 1);
    assert_eq!(p["counts"]["not_run"], 1);
}
