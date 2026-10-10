//! Development-only member and group exclusions keep a possible call separate from deployment.
#![cfg(feature = "dev")]

mod common;

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use terobacter::genomovar::{
    diagnostic_assess, Context, Definition, GenomovarParent, MemberRow, DEFINITION_VERSION, ENGINE,
    ENGINE_VERSION, PARENT_SPECIES_ID, PARENT_SUBSPECIES_ID, QUERY_SCOPE, REFERENCE_SCOPE,
    SEARCH_PARAMETERS,
};

const FIXTURE: &[u8] = include_bytes!("fixtures/genomovar-diagnostics/cases.json");
const FIXTURE_SHA256: &str = "1434bef5e2f33de2630aa8651193d943d88f3a02749ee7f26b534451cc38206e";

#[derive(Debug, Deserialize)]
struct FixtureDocument {
    cases: Vec<FixtureCase>,
}

#[derive(Debug, Deserialize)]
struct FixtureCase {
    id: String,
    call: String,
    groups: BTreeMap<String, Vec<String>>,
    source_group: String,
    rival: Option<String>,
    rows: Vec<FixtureRow>,
    baseline: Value,
    #[serde(default)]
    removed_rows: Vec<String>,
    #[serde(default)]
    masked_members: Vec<String>,
    #[serde(default)]
    removed_groups: Vec<String>,
    applicable: Option<bool>,
    refused: Option<String>,
    #[serde(default)]
    expected: Value,
}

#[derive(Debug, Deserialize)]
struct FixtureRow {
    reference_id: String,
    ani: String,
    af_query: String,
    af_reference: String,
}

fn context() -> Context {
    let query_sha256 = "a".repeat(64);
    let package_sha256 = "b".repeat(64);
    Context {
        parent: GenomovarParent {
            policy_id: terobacter::decision::V1_POLICY_ID.into(),
            policy_sha256: String::new(),
            result_sha256: String::new(),
            status: "assigned".into(),
            species_id: Some(PARENT_SPECIES_ID.into()),
            subspecies_id: Some(PARENT_SUBSPECIES_ID.into()),
            verified: true,
            query_sha256: query_sha256.clone(),
            package_sha256: package_sha256.clone(),
            blocking_reasons: Vec::new(),
            scope_outcome: "not_assessed".into(),
        },
        input_kind: "assembly".into(),
        query_scope: QUERY_SCOPE.into(),
        reference_scope: REFERENCE_SCOPE.into(),
        engine: ENGINE.into(),
        engine_version: ENGINE_VERSION.into(),
        parameters: SEARCH_PARAMETERS.into(),
        search_status: "completed".into(),
        adapter_verified: true,
        query_sha256,
        package_sha256,
        diagnostic_exclusion: false,
    }
}

fn apply_case(case: &FixtureCase) -> terobacter::genomovar::Assessment {
    let definition = definition_for_case(case);
    diagnostic_assess(
        &definition,
        &rows_for_case(case),
        &context(),
        &case.masked_members,
        &case.removed_groups,
        &case.removed_rows,
    )
    .unwrap_or_else(|error| panic!("{}: {error}", case.id))
}

fn definition_for_case(case: &FixtureCase) -> Definition {
    Definition::new(DEFINITION_VERSION, case.groups.clone()).unwrap()
}

fn rows_for_case(case: &FixtureCase) -> Vec<MemberRow> {
    case
        .rows
        .iter()
        .map(|row| {
            serde_json::from_value(json!({
                "reference_id": row.reference_id,
                "ani_percent": row.ani.parse::<serde_json::Number>().unwrap(),
                "aligned_fraction_query_percent": row.af_query.parse::<serde_json::Number>().unwrap(),
                "aligned_fraction_reference_percent": row.af_reference.parse::<serde_json::Number>().unwrap()
            }))
            .unwrap()
        })
        .collect()
}

fn measured_contradictions(case: &FixtureCase, group: &str) -> usize {
    let group_members: std::collections::BTreeSet<_> =
        case.groups[group].iter().map(String::as_str).collect();
    let missing: std::collections::BTreeSet<_> =
        case.removed_rows.iter().map(String::as_str).collect();
    case.rows
        .iter()
        .filter(|row| {
            group_members.contains(row.reference_id.as_str())
                && !missing.contains(row.reference_id.as_str())
                && row.ani.parse::<f64>().unwrap() < 99.5
        })
        .count()
}

#[test]
fn accepted_gv4_removal_fixtures_keep_diagnostic_and_deployment_fields_separate() {
    assert_eq!(format!("{:x}", Sha256::digest(FIXTURE)), FIXTURE_SHA256);
    let cases: FixtureDocument = serde_json::from_slice(FIXTURE).unwrap();
    assert_eq!(cases.cases.len(), 17);

    let (mut applicable, mut refused, mut not_applicable) = (0, 0, 0);
    for case in &cases.cases {
        if case.applicable == Some(false) {
            // Not-applicable controls describe why no diagnostic scenario should be run.
            assert!(case.refused.is_none(), "{}", case.id);
            assert!(
                case.expected
                    .as_str()
                    .is_some_and(|text| text.starts_with("not_applicable_")),
                "{}: {}",
                case.id,
                case.expected
            );
            assert!(case.baseline["compatible_groups"].is_array(), "{}", case.id);
            assert!(case.baseline["incomplete_groups"].is_array(), "{}", case.id);
            assert_eq!(
                case.baseline["would_assign"], case.source_group,
                "{}",
                case.id
            );
            match case.expected.as_str().unwrap() {
                "not_applicable_no_rival" => assert!(case
                    .groups
                    .keys()
                    .filter(|group| *group != &case.source_group)
                    .all(|group| case.removed_groups.contains(group))),
                "not_applicable_no_second_contradiction" => assert!(case
                    .groups
                    .keys()
                    .filter(|group| *group != &case.source_group)
                    .all(|group| measured_contradictions(case, group) < 2)),
                expected => panic!("{}: unknown not-applicable result {expected}", case.id),
            }
            not_applicable += 1;
            continue;
        }

        if let Some(refusal) = case.refused.as_deref() {
            // Refused controls are not diagnostic inputs; preserve their supplied baseline
            // summary and validate the precondition that made each perturbation invalid.
            assert_eq!(case.applicable, None, "{}", case.id);
            assert!(case.baseline["compatible_groups"].is_array(), "{}", case.id);
            assert!(case.baseline["incomplete_groups"].is_array(), "{}", case.id);
            match refusal {
                "rival_without_second_contradiction" => {
                    let rival = case.rival.as_deref().expect("named refused rival");
                    assert_eq!(case.call, "drop_one_contradiction", "{}", case.id);
                    assert_eq!(
                        case.baseline["would_assign"], case.source_group,
                        "{}",
                        case.id
                    );
                    assert!(measured_contradictions(case, rival) < 2, "{}", case.id);
                }
                "rival_without_measured_contradiction" => {
                    assert_eq!(case.call, "missing_rival", "{}", case.id);
                    assert_eq!(case.rival, None, "{}", case.id);
                    assert_eq!(
                        case.baseline["would_assign"],
                        Value::Null,
                        "{}: missing-rival refusal requires no baseline assignment",
                        case.id
                    );
                    assert!(case.baseline["incomplete_groups"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|group| measured_contradictions(case, group) == 0));
                }
                other => panic!("{}: unknown refusal {other}", case.id),
            }
            refused += 1;
            continue;
        }

        let assessment = apply_case(case);
        assert_eq!(
            assessment.assigned_genomovar_id(),
            None,
            "{}: a diagnostic must never set the deployment field",
            case.id
        );
        assert_eq!(
            assessment.to_value()["genomovar_id"],
            Value::Null,
            "{}",
            case.id
        );
        assert_eq!(
            assessment.to_value()["diagnostic_would_assign_genomovar_id"],
            assessment
                .would_assign_genomovar_id()
                .map(|id| Value::String(id.to_owned()))
                .unwrap_or(Value::Null),
            "{}",
            case.id
        );
        assert_eq!(
            assessment.to_value()["diagnostic_actions"]["masked_members"],
            json!(case.masked_members),
            "{}",
            case.id
        );
        assert_eq!(
            assessment.to_value()["diagnostic_actions"]["removed_groups"],
            json!(case.removed_groups),
            "{}",
            case.id
        );
        assert_eq!(
            assessment.to_value()["diagnostic_actions"]["excluded_evidence_rows"],
            json!(case.removed_rows),
            "{}",
            case.id
        );
        assert!(assessment.to_value()["reasons"]
            .as_array()
            .unwrap()
            .contains(&json!("diagnostic_only_no_deployment_call")));

        assert_eq!(case.applicable, Some(true), "{}", case.id);
        assert_eq!(
            assessment.would_assign_genomovar_id(),
            case.expected["would_assign"].as_str(),
            "{}",
            case.id
        );
        assert_eq!(
            assessment.to_value()["incomplete_groups"],
            case.expected["incomplete_groups"],
            "{}",
            case.id
        );
        applicable += 1;
    }

    assert_eq!((applicable, refused, not_applicable), (12, 3, 2));
}

#[test]
fn valid_parent_withholding_is_a_diagnostic_outcome() {
    let cases: FixtureDocument = serde_json::from_slice(FIXTURE).unwrap();
    let case = cases
        .cases
        .iter()
        .find(|case| case.applicable == Some(true))
        .unwrap();
    let mut withheld = context();
    withheld.parent.status = "unresolved".into();
    withheld
        .parent
        .blocking_reasons
        .push("external_problem_warning".into());
    let assessment = diagnostic_assess(
        &definition_for_case(case),
        &rows_for_case(case),
        &withheld,
        &[],
        &[],
        &[],
    )
    .unwrap();
    let value = assessment.to_value();
    assert_eq!(assessment.would_assign_genomovar_id(), None);
    assert_eq!(assessment.assigned_genomovar_id(), None);
    assert_eq!(value["diagnostic_outcome"], "gv_withheld_parent");
    assert!(value["reasons"]
        .as_array()
        .unwrap()
        .contains(&json!("upstream_veto")));
}

#[test]
fn masked_parent_checks_original_evidence_before_exclusion() {
    use terobacter::genomovar::diagnostic_parent;
    use terobacter::{evidence::SavedEvidence, rescue::AnimRecords};
    let synthetic = common::package::Synthetic::new();
    let package = synthetic.verify().unwrap();
    let evidence = common::packaged_evidence(&synthetic.manifest_sha256());
    let member = evidence["ranked_reference_evidence"][0]["reference_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let saved = SavedEvidence::from_json(&evidence.to_string()).unwrap();
    for policy in [
        terobacter::genomovar::DiagnosticParentPolicy::V1,
        terobacter::genomovar::DiagnosticParentPolicy::V2(
            terobacter::rescue::Thresholds::standard(),
        ),
    ] {
        let result = diagnostic_parent(
            saved.clone(),
            &package,
            std::slice::from_ref(&member),
            &policy,
            &AnimRecords::default(),
            None,
        )
        .unwrap();
        assert_eq!(
            result["input"]["mask"]["excluded_reference_ids"],
            json!([member])
        );
        assert!(diagnostic_parent(
            saved.clone(),
            &package,
            &[member.clone(), member.clone()],
            &policy,
            &AnimRecords::default(),
            None
        )
        .is_err());
        let mut inconsistent = evidence.clone();
        inconsistent["ranked_reference_evidence"][0]["subspecies_name"] =
            "Incorrect metadata".into();
        let inconsistent = SavedEvidence::from_json(&inconsistent.to_string()).unwrap();
        assert!(diagnostic_parent(
            inconsistent,
            &package,
            std::slice::from_ref(&member),
            &policy,
            &AnimRecords::default(),
            None
        )
        .is_err());
    }
}

#[test]
fn v2_parent_recomputation_keeps_custom_settings_and_their_validation_mode() {
    use terobacter::genomovar::{diagnostic_parent, DiagnosticParentPolicy};
    let synthetic = common::package::Synthetic::new();
    let package = synthetic.verify().unwrap();
    let evidence = common::packaged_evidence(&synthetic.manifest_sha256());
    let saved = terobacter::evidence::SavedEvidence::from_json(&evidence.to_string()).unwrap();
    let source = terobacter::rescue::Thresholds::custom(Some("96.0000"), Some("98"))
        .unwrap()
        .policy_value();
    let policy = DiagnosticParentPolicy::from_report(&source).unwrap();
    let result = diagnostic_parent(
        saved,
        &package,
        &[],
        &policy,
        &terobacter::rescue::AnimRecords::default(),
        None,
    )
    .unwrap();
    for field in ["policy_mode", "settings_sha256", "thresholds"] {
        assert_eq!(result[field], source[field]);
    }
    assert!(!GenomovarParent::from_report(&result)
        .eligibility_reasons()
        .is_empty());
    let mut inconsistent = source;
    inconsistent["settings_sha256"] = "f".repeat(64).into();
    assert!(DiagnosticParentPolicy::from_report(&inconsistent).is_err());
}
