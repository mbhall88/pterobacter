mod common;
use common::{decision_from_text, evidence_json, row};
use serde_json::{json, Value};
use terobacter::{
    decision::{decide, Policy},
    evidence::SavedEvidence,
};

fn annotated_evidence() -> Value {
    let mut e = evidence_json(&[
        row("ref-a", Some("SP1"), 99.0, 90.0, 90.0),
        row("ref-b", Some("SP2"), 96.0, 10.0, 80.0),
    ]);
    e["assignment"]["status"] = "unresolved".into();
    e["assignment"]["reason"] = "unexplained_residual_sequence".into();
    e["scope_assessment"] = json!({"schema_version":"1.0.0-scope-assessment",
        "query_sha256":"b".repeat(64), "evidence_sha256":"c".repeat(64),
        "outcome":"unexplained_after_checks", "reason":"Extra sequence remains unexplained",
        "checks":[{"check_id":"scope-1","method":"synthetic-check","method_version":"1",
            "artifact_path":"/synthetic/scope.txt","sha256":"d".repeat(64),"status":"completed"}]});
    e
}
fn warning(e: &SavedEvidence) -> Value {
    json!({"schema_version":"1.0-shadow-problem-warnings", "query_sha256":"b".repeat(64),
        "reference_package_sha256":"a".repeat(64), "evidence_sha256":e.source_sha256(),
        "warnings":[{"warning_id":"problem-1","reason":"Supplied concern",
            "source":"synthetic-test","source_version":"1",
            "artifacts":[{"path":"/synthetic/warning.txt","sha256":"e".repeat(64)}]}]})
}
#[test]
fn simultaneous_scope_parent_and_external_warning_vetoes_are_all_retained() {
    let e = SavedEvidence::from_json(&annotated_evidence().to_string()).unwrap();
    let envelope = warning(&e).to_string();
    let (method, inventory) = method_inputs(&e, "ref-a", json!(95.99), json!(90), json!(90));
    let e = e
        .with_annotations(
            Some(&method.to_string()),
            Some(&envelope),
            Some(&inventory.to_string()),
        )
        .unwrap();
    let d = decide(&e, &Policy::v1());
    let d = d.to_value();
    assert_eq!(
        d["blocking_reasons"],
        json!([
            "two_parent_support_alert",
            "unexplained_residual_sequence",
            "coverage_method_disagreement",
            "external_problem_warning"
        ])
    );
    assert_eq!(d["assignment"]["reason"], "unexplained_residual_sequence");
    assert_eq!(d["assignment"]["species_id"], Value::Null);
    assert_eq!(
        d["external_problem_warnings"][0]["reason"],
        "Supplied concern"
    );
    assert_eq!(d["scope_assessment"]["checks"][0]["check_id"], "scope-1");
}
#[test]
fn an_external_warning_bound_to_another_query_is_rejected() {
    let e = SavedEvidence::from_json(&annotated_evidence().to_string()).unwrap();
    let mut envelope = warning(&e);
    envelope["query_sha256"] = "f".repeat(64).into();
    assert!(e
        .with_annotations(None, Some(&envelope.to_string()), None)
        .is_err());
}
#[test]
fn missing_assessment_is_visible_and_does_not_establish_purity() {
    let e = evidence_json(&[
        row("ref-a", Some("SP1"), 99.0, 90.0, 90.0),
        row("ref-b", Some("SP2"), 90.0, 90.0, 90.0),
    ]);
    let d = decision_from_text(&e.to_string());
    assert_eq!(d["scope_assessment"]["outcome"], "not_assessed");
    assert_eq!(d["purity_established"], false);
}

fn method_inputs(
    e: &SavedEvidence,
    reference: &str,
    ani: Value,
    aq: Value,
    ar: Value,
) -> (Value, Value) {
    let r = e.references().iter().find(|r| r.id == reference).unwrap();
    let triple = json!({"ani_percent":ani,"af_query_percent":aq,"af_reference_percent":ar});
    let mut directions = vec![];
    for id in ["reference_to_query", "query_to_reference"] {
        let mut direction = triple.clone();
        direction["direction_id"] = id.into();
        directions.push(direction);
    }
    let diagnostic = json!({"schema_version":"1.0-shadow-diagnostics",
        "query_sha256":e.provenance().query_sha256,"evidence_sha256":e.source_sha256(),
        "reference_package_sha256":e.provenance().reference_package_sha256,
        "diagnostics":[{"diagnostic_id":"method-1","method":"reciprocal_ANIm","method_version":"synthetic-1",
            "reference_id":reference,"reference_source_sha256":"f".repeat(64),
            "query_scope":"whole_assembly_as_supplied","reference_scope":"whole_assembly_as_deposited","skani_version":"0.3.2",
            "skani":{"ani_percent":r.ani.as_f64(),"af_query_percent":r.af_query.as_f64(),"af_reference_percent":r.af_reference.as_f64()},
            "directions":directions,"artifacts":[{"path":"/synthetic/method.txt","sha256":"e".repeat(64)}]}]});
    let references: Vec<_> = e.references().iter().map(|r| json!({"reference_id":r.id,"source_sha256":"f".repeat(64),
        "species_id":r.species.as_ref().map_or("",|s| s.id.as_str()),"species_name":r.species.as_ref().map_or("",|s| s.name.as_str()),
        "subspecies_id":r.subspecies.as_ref().map_or("",|s| s.id.as_str()),"subspecies_name":r.subspecies.as_ref().map_or("",|s| s.name.as_str()),
        "lineage_id":r.lineage_id.as_deref().unwrap_or(""),"reference_role":r.reference_role,"name_status":r.name_status})).collect();
    let inventory = json!({"schema_version":"1.0-shadow-primary-inventory","component_id":"primary_whole",
        "package_sha256":e.provenance().reference_package_sha256,"references":references,"representation":{}});
    (diagnostic, inventory)
}
fn simple() -> SavedEvidence {
    SavedEvidence::from_json(
        &evidence_json(&[
            row("ref-a", Some("SP1"), 99.0, 90.0, 90.0),
            row("ref-b", Some("SP2"), 90.0, 90.0, 90.0),
        ])
        .to_string(),
    )
    .unwrap()
}
#[test]
fn reciprocal_method_disagreement_and_foreign_parent_support_both_withhold() {
    for (reference, ani) in [("ref-a", 95.99), ("ref-b", 96.0)] {
        let e = simple();
        let (d, i) = method_inputs(&e, reference, json!(ani), json!(80), json!(80));
        let e = e
            .with_annotations(Some(&d.to_string()), None, Some(&i.to_string()))
            .unwrap();
        let decision = decide(&e, &Policy::v1());
        let d = decision.to_value();
        assert_eq!(d["assignment"]["reason"], "coverage_method_disagreement");
        assert_eq!(d["assignment"]["species_id"], Value::Null);
        assert_eq!(d["method_diagnostics"][0]["gate_disagreement"], true);
    }
}
#[test]
fn method_decimals_are_not_rounded_onto_a_gate_and_allow_finer_precision() {
    for (ani, expected) in [
        ("95.999999999999999", true),
        ("96.000000000000001", false),
        ("9.6e1", false),
    ] {
        let e = simple();
        let (mut d, i) = method_inputs(&e, "ref-a", json!(96), json!(80), json!(80));
        for x in d["diagnostics"][0]["directions"].as_array_mut().unwrap() {
            x["ani_percent"] = ani.into();
        }
        let e = e
            .with_annotations(Some(&d.to_string()), None, Some(&i.to_string()))
            .unwrap();
        assert_eq!(
            decide(&e, &Policy::v1()).to_value()["method_diagnostics"][0]["gate_disagreement"],
            expected
        );
    }
}
#[test]
fn one_failing_reciprocal_direction_blocks_even_when_the_other_agrees() {
    let e = simple();
    let (mut d, i) = method_inputs(&e, "ref-a", json!(99), json!(90), json!(90));
    d["diagnostics"][0]["directions"][1]["af_query_percent"] = 79.99.into();
    let e = e
        .with_annotations(Some(&d.to_string()), None, Some(&i.to_string()))
        .unwrap();
    assert_eq!(
        decide(&e, &Policy::v1()).to_value()["blocking_reasons"],
        json!(["coverage_method_disagreement"])
    );
}
#[test]
fn invalid_method_bindings_are_errors_instead_of_negative_measurements() {
    for field in ["query_scope", "reference_source_sha256", "skani_version"] {
        let e = simple();
        let (mut d, i) = method_inputs(&e, "ref-a", json!(99), json!(90), json!(90));
        d["diagnostics"][0][field] = "wrong".into();
        assert!(
            e.with_annotations(Some(&d.to_string()), None, Some(&i.to_string()))
                .is_err(),
            "{field}"
        );
    }
    let e = simple();
    let (mut d, i) = method_inputs(&e, "ref-a", json!(99), json!(90), json!(90));
    d["diagnostics"][0]["directions"][1]["direction_id"] = "reference_to_query".into();
    assert!(e
        .with_annotations(Some(&d.to_string()), None, Some(&i.to_string()))
        .is_err());
}
#[test]
fn incomplete_parent_evidence_and_scope_veto_keep_both_reasons() {
    let mut e = annotated_evidence();
    e["ranked_reference_evidence"][1]["species_id"] = "SP1".into();
    e["ranked_reference_evidence"][1]["species_name"] = "Enterobacter SP1".into();
    let d = decision_from_text(&e.to_string());
    assert_eq!(d["evaluation_status"], "incomplete");
    assert_eq!(d["eligibility_state"], "screened_ineligible");
    assert_eq!(
        d["blocking_reasons"],
        json!([
            "unexplained_residual_sequence",
            "species_competitor_unmeasured"
        ])
    );
}
#[test]
fn an_explained_scope_cannot_clear_an_external_problem_warning() {
    let mut e = annotated_evidence();
    e["scope_assessment"]["outcome"] = "explained_after_checks".into();
    e["assignment"]["reason"] = "development_evidence_only".into();
    let e = SavedEvidence::from_json(&e.to_string()).unwrap();
    let w = warning(&e).to_string();
    let e = e.with_annotations(None, Some(&w), None).unwrap();
    assert_eq!(
        decide(&e, &Policy::v1()).to_value()["blocking_reasons"],
        json!(["two_parent_support_alert", "external_problem_warning"])
    );
    assert!(e.with_annotations(None, None, None).is_err());
}
