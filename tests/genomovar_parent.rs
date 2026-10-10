//! The genomovar parent: an actually assigned `ECTX-SP025` species and `ECTX-SS007` subspecies,
//! read from a final v1 or v2 decision. The v2 bridge accepts only a call that is the unchanged
//! v1 skani call on the same evidence; an ANIm species rescue leaves subspecies null and never
//! supports a genomovar.
mod common;

use std::collections::BTreeMap;

use common::p1;
use serde_json::{json, Value};
use terobacter::decision::{decide, Policy};
use terobacter::evidence::SavedEvidence;
use terobacter::genomovar::{
    assess, Context, Definition, GenomovarParent, MemberRow, DEFINITION_VERSION,
};

const SP: &str = "ECTX-SP025";
const SS: &str = "ECTX-SS007";

fn reference(id: &str, species: &str, subspecies: Option<&str>, ani: &str) -> Value {
    json!({"id": id, "species": species, "subspecies": subspecies, "ani": ani, "aq": "85", "ar": "85"})
}

/// A Group A query: 99.9 to a hoffmannii reference, 98.5 to another subspecies, 90 elsewhere.
fn group_a() -> Value {
    json!({"references": [
        reference("A1", SP, Some(SS), "99.9"),
        reference("A2", SP, Some("ECTX-SS001"), "98.5"),
        reference("B1", "ECTX-SP001", None, "90")], "anim": {}})
}

/// A borderline query rescued by ANIm through a hoffmannii witness.
fn rescued() -> Value {
    json!({"references": [
        reference("A1", SP, Some(SS), "95.5"),
        reference("B1", "ECTX-SP001", None, "90")],
        "anim": {"A1": p1::reciprocal("96.2", "85", "85"), "B1": p1::reciprocal("90", "85", "85")}})
}

fn v1(input: &Value, warnings: &[&str]) -> Value {
    let evidence = SavedEvidence::from_json(&p1::evidence_text("parent", input)).unwrap();
    let evidence = if warnings.is_empty() {
        evidence
    } else {
        let text = p1::warnings_text(&evidence, warnings);
        evidence.with_annotations(None, Some(&text), None).unwrap()
    };
    bound(decide(&evidence, &Policy::v1()).to_value().clone())
}

/// The binding a native route adds once it decides against the authenticated package, as
/// `decide_bound` reports it.
fn bound(mut decision: Value) -> Value {
    decision["reference_package"] =
        json!({"status": "verified", "package": {"manifest_sha256": p1::PACKAGE_SHA256}});
    decision
}

fn unbound_v2(input: &Value) -> Value {
    p1::decide(&json!({"id": "parent", "input": input})).unwrap()
}

fn v2(input: &Value) -> Value {
    bound(unbound_v2(input))
}

/// Whether the parent would let an otherwise unique, fully measured group be assigned.
fn supports_genomovar(parent: &GenomovarParent) -> bool {
    let definition = Definition::new(
        DEFINITION_VERSION,
        BTreeMap::from([("GV01".to_owned(), vec!["m".to_owned()])]),
    )
    .unwrap();
    let row: MemberRow = serde_json::from_value(json!({"reference_id": "m", "ani_percent": 100,
        "aligned_fraction_query_percent": 90, "aligned_fraction_reference_percent": 90}))
    .unwrap();
    let context = Context {
        parent: parent.clone(),
        input_kind: "assembly".into(),
        query_scope: "whole_assembly_as_supplied".into(),
        reference_scope: "historical_chromosome_record_set".into(),
        engine: "skani".into(),
        engine_version: "0.3.2".into(),
        parameters: "c125_m1000_learnedANI_s80_minAF0_uncapped".into(),
        search_status: "completed".into(),
        adapter_verified: true,
        query_sha256: p1::QUERY_SHA256.into(),
        package_sha256: p1::PACKAGE_SHA256.into(),
        diagnostic_exclusion: false,
    };
    assess(&definition, &[row], &context)
        .unwrap()
        .assigned_genomovar_id()
        .is_some()
}

#[test]
fn an_assigned_v1_group_a_call_is_an_eligible_parent() {
    let parent = GenomovarParent::from_report(&v1(&group_a(), &[]));
    assert_eq!(parent.policy_id, "conservative-assembly-v1");
    assert_eq!(parent.species_id.as_deref(), Some(SP));
    assert_eq!(parent.subspecies_id.as_deref(), Some(SS));
    assert_eq!(parent.query_sha256, p1::QUERY_SHA256);
    assert_eq!(parent.package_sha256, p1::PACKAGE_SHA256);
    assert!(supports_genomovar(&parent));
}

#[test]
fn a_v1_call_without_a_verified_package_binding_is_not_a_parent() {
    let mut decision = v1(&group_a(), &[]);
    decision["reference_package"] = json!({"status": "not_verified"});
    assert!(!supports_genomovar(&GenomovarParent::from_report(
        &decision
    )));
    let mut decision = v1(&group_a(), &[]);
    decision["reference_package"]["package"]["manifest_sha256"] = "c".repeat(64).into();
    assert!(!supports_genomovar(&GenomovarParent::from_report(
        &decision
    )));
}

#[test]
fn a_vetoed_v1_call_is_not_a_parent() {
    let parent = GenomovarParent::from_report(&v1(&group_a(), &["concern"]));
    assert_eq!(parent.status, "unresolved");
    assert_eq!(parent.blocking_reasons, vec!["external_problem_warning"]);
    assert!(!supports_genomovar(&parent));
}

#[test]
fn another_subspecies_or_a_species_only_call_is_not_a_parent() {
    let mut other = group_a();
    other["references"][0]["subspecies"] = "ECTX-SS001".into();
    other["references"][1]["subspecies"] = SS.into();
    let parent = GenomovarParent::from_report(&v1(&other, &[]));
    assert_eq!(parent.subspecies_id.as_deref(), Some("ECTX-SS001"));
    assert!(!supports_genomovar(&parent));

    // Within the one-point subspecies margin: species assigned, subspecies withheld.
    let mut close = group_a();
    close["references"][1]["ani"] = "99.5".into();
    let parent = GenomovarParent::from_report(&v1(&close, &[]));
    assert_eq!(parent.species_id.as_deref(), Some(SP));
    assert_eq!(parent.subspecies_id, None);
    assert!(!supports_genomovar(&parent));
}

#[test]
fn a_v2_skani_call_bridges_to_the_same_v1_parent() {
    let decision = v2(&group_a());
    assert_eq!(decision["call_basis"], "skani");
    let parent = GenomovarParent::from_report(&decision);
    assert_eq!(parent.policy_id, "conservative-assembly-anim-v2");
    assert_eq!(parent.subspecies_id.as_deref(), Some(SS));
    assert!(supports_genomovar(&parent));
    let legacy = GenomovarParent::from_report(&v1(&group_a(), &[]));
    assert_eq!(
        (&parent.species_id, &parent.subspecies_id),
        (&legacy.species_id, &legacy.subspecies_id)
    );
}

#[test]
fn a_v2_call_without_a_verified_package_binding_is_not_a_parent() {
    // A pure v2 decision checks only its contract, not the package its evidence names.
    let decision = unbound_v2(&group_a());
    assert_eq!(decision["call_basis"], "skani");
    assert!(!supports_genomovar(&GenomovarParent::from_report(
        &decision
    )));
    let mut decision = v2(&group_a());
    decision["reference_package"]["package"]["manifest_sha256"] = "c".repeat(64).into();
    assert!(!supports_genomovar(&GenomovarParent::from_report(
        &decision
    )));
}

#[test]
fn a_rescued_species_has_null_subspecies_and_no_genomovar_parent() {
    let decision = v2(&rescued());
    assert_eq!(decision["call_basis"], "ANIm_assisted");
    assert_eq!(decision["species_id"], SP);
    assert_eq!(decision["subspecies_id"], Value::Null);
    let parent = GenomovarParent::from_report(&decision);
    assert_eq!(parent.status, "assigned");
    assert_eq!(parent.subspecies_id, None);
    assert!(!supports_genomovar(&parent));

    // Even a rescue that somehow carried the subspecies is not the unchanged v1 call.
    let mut forged = decision.clone();
    forged["subspecies_id"] = SS.into();
    assert!(!supports_genomovar(&GenomovarParent::from_report(&forged)));
}

#[test]
fn a_v2_call_that_disagrees_with_its_v1_result_is_not_a_parent() {
    let mut decision = v2(&group_a());
    decision["legacy_v1_result"]["subspecies_id"] = "ECTX-SS001".into();
    assert!(!supports_genomovar(&GenomovarParent::from_report(
        &decision
    )));
}

#[test]
fn custom_v2_thresholds_are_not_a_parent_even_at_default_values() {
    let mut input = group_a();
    input["custom"] = json!({"species_ani": "96.0"});
    let decision = v2(&input);
    assert_eq!(decision["policy_mode"], "custom");
    assert_eq!(decision["subspecies_id"], SS);
    assert!(!supports_genomovar(&GenomovarParent::from_report(
        &decision
    )));
}

#[test]
fn a_v2_method_veto_withholds_the_parent() {
    let mut input = group_a();
    input["anim"] = json!({"A1": p1::reciprocal("99.9", "70", "70")});
    let decision = v2(&input);
    assert_eq!(decision["status"], "unresolved", "{decision}");
    assert!(!supports_genomovar(&GenomovarParent::from_report(
        &decision
    )));
}

#[test]
fn an_unknown_report_shape_fails_closed() {
    for report in [
        json!({}),
        json!({"status": "assigned", "species_id": SP, "subspecies_id": SS}),
    ] {
        assert!(!supports_genomovar(&GenomovarParent::from_report(&report)));
    }
}
