//! A primary-reference mask removes references from the skani evidence and from the ANIm
//! competitor set, for v2 and for the legacy v1 result reported beside it.
mod common;

use common::p1::{self, hit, reciprocal, unreturned, PROFILE};
use serde_json::{json, Value};

fn decide(input: &Value, mask: &[&str]) -> Value {
    let (rescue, records) = p1::input("mask", input, mask).unwrap();
    let anim = rescue.bind_supplied(&records, PROFILE).unwrap();
    rescue.decide(&anim).to_value().clone()
}

/// A rescue in species A that an unreturned species-C reference challenges through ANIm.
fn challenged() -> Value {
    json!({
        "references": [hit("A1", Some("A"), "95.5", "85", "85"), hit("B1", Some("B"), "90", "85", "85"),
            unreturned("C1", Some("C"))],
        "anim": {"A1": reciprocal("96.2", "85", "85"), "B1": reciprocal("90", "85", "85"),
            "C1": reciprocal("96.5", "85", "85")}})
}

#[test]
fn a_masked_competitor_leaves_the_anim_competitor_set() {
    let unmasked = decide(&challenged(), &[]);
    assert_eq!(unmasked["challenges"], json!(["C1"]));
    assert_eq!(unmasked["species_id"], Value::Null);
    assert_eq!(unmasked["counts"]["required"], 3);

    let masked = decide(&challenged(), &["C1"]);
    assert_eq!(masked["challenges"], json!([]));
    assert_eq!(masked["species_id"], "A");
    assert_eq!(masked["call_basis"], "ANIm_assisted");
    assert_eq!(masked["competitor_evaluation"], "complete");
    assert_eq!(masked["counts"]["required"], 2);
    assert_eq!(
        masked["input"]["mask"]["excluded_reference_ids"],
        json!(["C1"])
    );
    assert_eq!(masked["input"]["mask"]["panel_reference_count"], 3);
    assert_eq!(masked["input"]["mask"]["assessed_panel_reference_count"], 2);
    assert_eq!(
        masked["input"]["anim"]["excluded"],
        json!([{"reference_id": "C1", "reason": "masked_reference"}])
    );
}

#[test]
fn a_masked_competitor_needs_no_anim_for_a_complete_panel() {
    let mut input = challenged();
    input["anim"].as_object_mut().unwrap().remove("C1");
    assert_eq!(
        decide(&input, &[])["reasons"],
        json!(["ANIm_competitor_incomplete"])
    );
    assert_eq!(decide(&input, &["C1"])["species_id"], "A");
}

#[test]
fn a_masked_skani_row_leaves_the_evidence_for_v2_and_legacy_v1() {
    // Without the mask, A0 at 96.5 is an ordinary v1 call. Masking it leaves A1 at 95.5 nearest.
    let mut input = challenged();
    input["references"]
        .as_array_mut()
        .unwrap()
        .insert(0, hit("A0", Some("A"), "96.5", "85", "85"));
    input["anim"].as_object_mut().unwrap().remove("C1");
    input["anim"]["C1"] = reciprocal("90", "85", "85");
    input["anim"]["A0"] = reciprocal("96.5", "85", "85");

    // Unmasked, A1's supplied ANIm would cross the old gate outside the band and veto, so the
    // ordinary call is checked from skani alone.
    let mut skani_only = input.clone();
    skani_only["anim"] = json!({});
    let unmasked = decide(&skani_only, &[]);
    assert_eq!(unmasked["trigger"], false);
    assert_eq!(unmasked["call_basis"], "skani");
    assert_eq!(unmasked["legacy_v1_result"]["status"], "assigned");

    let masked = decide(&input, &["A0"]);
    assert_eq!(masked["trigger"], true);
    assert_eq!(masked["max_skani_ani_percent"], 95.5);
    assert_eq!(masked["call_basis"], "ANIm_assisted");
    assert_eq!(masked["legacy_v1_result"]["status"], "unresolved");
    assert_eq!(
        masked["legacy_v1_result"]["reasons"],
        json!(["nearest_below_species_ANI"])
    );
    let shown: Vec<_> = masked["top_references"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["reference_id"].as_str().unwrap())
        .collect();
    assert_eq!(shown, ["A1", "B1"]);
    let panel_counts: u64 = masked["panel_summary"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["panel_reference_count"].as_u64().unwrap())
        .sum();
    assert_eq!(panel_counts, 3);
}

#[test]
fn a_mask_must_name_distinct_panel_references() {
    for mask in [&["Z9"][..], &["B1", "B1"][..]] {
        let error = p1::input("mask", &challenged(), mask).err().unwrap();
        assert!(error.to_string().contains("mask"), "{error}");
    }
}
