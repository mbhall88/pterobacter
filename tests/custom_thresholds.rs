//! Exact custom-policy endpoints through the agreed R2/R4 interfaces.
mod common;
use common::p1::{self, hit, reciprocal, PROFILE};
use serde_json::{json, Value};
use terobacter::rescue::{BoundAnim, Thresholds};

fn input(t: &str, s: &str, ani: &str) -> Value {
    json!({"custom": {"species_ani": t, "subspecies_ani": s},
        "references": [hit("A1", Some("A"), ani, "80", "80"),
            hit("B1", Some("B"), "79", "85", "85")]})
}
fn decide(input: &Value) -> Value {
    let (r, records) = p1::input("custom", input, &[]).unwrap();
    let bound = r.bind_supplied(&records, PROFILE).unwrap();
    r.decide(&bound).unwrap().to_value().clone()
}
fn plan(input: &Value) -> Value {
    let (r, _) = p1::input("custom", input, &[]).unwrap();
    r.plan(&BoundAnim::none()).unwrap().to_value().clone()
}

#[test]
fn valid_ranges_include_both_endpoints_and_equal_species_subspecies() {
    for (t, s) in [
        ("80", "80"),
        ("80", "100"),
        ("100", "100"),
        ("96.000000000001", "98"),
        ("99.999999999999", "100"),
    ] {
        assert!(Thresholds::custom(Some(t), Some(s)).unwrap().is_custom());
    }
    for (t, s) in [
        ("79.999999999999", "98"),
        ("96", "100.000000000001"),
        ("96.000000000001", "96"),
        ("100.000000000001", "100"),
        ("80", "79.99"),
    ] {
        assert!(Thresholds::custom(Some(t), Some(s)).is_err(), "{t}/{s}");
    }
    for text in [
        "", "NaN", "inf", "-80", "96e0", "96.", ".96", " 96", "96 ", "9_6",
    ] {
        assert!(Thresholds::custom(Some(text), None).is_err(), "{text}");
    }
}

#[test]
fn trigger_endpoints_are_exact_inclusive_lower_and_exclusive_upper() {
    for (t, s, ani, expected, lower) in [
        ("80", "80", "80", false, "80"),
        ("81", "98", "79.99", false, "80"),
        ("81", "98", "80", true, "80"),
        ("81", "98", "80.99", true, "80"),
        ("81", "98", "81", false, "80"),
        ("95.505", "98", "94", false, "94.005"),
        ("95.505", "98", "94.01", true, "94.005"),
        ("95.505", "98", "95.5", true, "94.005"),
        ("95.505", "98", "95.51", false, "94.005"),
        ("100", "100", "98.49", false, "98.5"),
        ("100", "100", "98.5", true, "98.5"),
        ("100", "100", "100", false, "98.5"),
    ] {
        let p = plan(&input(t, s, ani));
        assert_eq!(p["trigger"], expected, "T={t}, ANI={ani}");
        assert_eq!(p["thresholds"]["trigger_lower_ani"], lower);
        assert_eq!(p["validation_status"], "custom_unvalidated");
    }
}

#[test]
fn both_anim_identities_must_strictly_exceed_custom_t_and_all_four_afs_pass() {
    let mut i = input("95.5", "98", "95");
    i["anim"] = json!({"A1": reciprocal("95.50000001", "80", "80"),
        "B1": reciprocal("79", "85", "85")});
    let pass = decide(&i);
    assert_eq!(pass["species_id"], "A");
    assert_eq!(pass["call_basis"], "ANIm_assisted");
    assert_eq!(pass["legacy_v1_result"]["species_id"], Value::Null);
    assert_eq!(pass["subspecies_id"], Value::Null);
    for direction in 0..2 {
        for (field, value) in [
            ("ani", "95.5"),
            ("ani", "95.49999999"),
            ("aq", "79.99"),
            ("ar", "79.99"),
        ] {
            let mut fail = i.clone();
            fail["anim"]["A1"]["directions"][direction][field] = json!(value);
            assert_eq!(
                decide(&fail)["species_id"],
                Value::Null,
                "{direction}/{field}/{value}"
            );
        }
    }
    let mut impossible = input("100", "100", "99");
    impossible["anim"] = json!({"A1": reciprocal("100", "80", "80"),
        "B1": reciprocal("79", "85", "85")});
    assert_eq!(decide(&impossible)["species_id"], Value::Null);
}

#[test]
fn foreign_challenge_uses_minimum_of_96_and_t_without_query_af_floor() {
    for (t, nearest, challenge) in [("95.5", "95", "95.5"), ("97", "96.5", "96")] {
        let mut i = input(t, "98", nearest);
        i["anim"] = json!({"A1": reciprocal("97.1", "80", "80"),
            "B1": reciprocal("90", "10", "80")});
        assert_eq!(decide(&i)["species_id"], "A");
        for direction in 0..2 {
            let mut challenged = i.clone();
            challenged["anim"]["B1"]["directions"][direction]["ani"] = json!(challenge);
            let d = decide(&challenged);
            assert_eq!(d["species_id"], Value::Null);
            assert!(d["reasons"]
                .as_array()
                .unwrap()
                .contains(&json!("foreign_parent_challenge")));
            assert_eq!(d["thresholds"]["conflict_ani"], challenge);
        }
    }
}

#[test]
fn custom_subspecies_threshold_retains_the_one_point_margin_and_af80() {
    for (best, second, aq, expected) in [
        ("96.99", "95.99", "80", Value::Null),
        ("97", "96", "80", json!("alpha")),
        ("97", "96.01", "80", Value::Null),
        ("97.01", "96.01", "80", json!("alpha")),
        ("97", "96", "79.99", Value::Null),
    ] {
        let mut a = hit("A1", Some("A"), best, aq, "80");
        a["subspecies"] = json!("alpha");
        let mut b = hit("A2", Some("A"), second, "80", "80");
        b["subspecies"] = json!("beta");
        let i = json!({"custom": {"species_ani": "95", "subspecies_ani": "97"},
            "references": [a, b, hit("B1", Some("B"), "90", "85", "85")]});
        assert_eq!(
            decide(&i)["subspecies_id"],
            expected,
            "{best}/{second}/{aq}"
        );
    }
}

#[test]
fn custom_rescue_preserves_raw_nearest_and_skani_coverage_requirements() {
    let mut i = input("97", "98", "96.5");
    i["references"][0]["aq"] = json!("79.99");
    i["references"][1] = hit("B1", Some("B"), "96", "80", "80");
    i["anim"] = json!({"A1": reciprocal("98", "80", "80"),
        "B1": reciprocal("98", "80", "80")});
    assert_eq!(decide(&i)["species_id"], Value::Null);
    assert!(plan(&i)["witness_candidates"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn long_decimal_thresholds_keep_exact_anim_and_range_comparisons() {
    let mut i = input("95.5000000000000000000001", "98", "95");
    i["anim"] = json!({"A1": reciprocal("95.5", "80", "80"),
        "B1": reciprocal("79", "85", "85")});
    assert_eq!(decide(&i)["species_id"], Value::Null);
    i["anim"]["A1"] = reciprocal("95.50000001", "80", "80");
    assert_eq!(decide(&i)["species_id"], "A");
    for (t, s) in [
        ("79.9999999999999999999999", "98"),
        ("96", "100.0000000000000000000001"),
        ("96.0000000000000000000001", "96"),
    ] {
        assert!(Thresholds::custom(Some(t), Some(s)).is_err());
    }
}
