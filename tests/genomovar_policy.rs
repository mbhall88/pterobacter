//! The accepted P3a genomovar contract (`p3a-genomovar-contract-r1`) against the production
//! policy. These are the worker specification's 20 tests, ported case for case, and the
//! coordinator review's 1,250 small-fixture policy cases. Expected values come from the
//! contract and its executable model, never from this implementation.
use std::collections::BTreeMap;

use serde_json::{json, Value};
use terobacter::genomovar::{assess, Context, Definition, MemberRow, Parent, DEFINITION_VERSION};

fn context() -> Context {
    Context {
        parent: Parent {
            policy_id: "conservative-assembly-v1".into(),
            status: "assigned".into(),
            species_id: Some("ECTX-SP025".into()),
            subspecies_id: Some("ECTX-SS007".into()),
            verified: true,
            query_sha256: "a".repeat(64),
            package_sha256: "f".repeat(64),
            blocking_reasons: vec![],
            scope_outcome: "not_assessed".into(),
        },
        input_kind: "assembly".into(),
        query_scope: "whole_assembly_as_supplied".into(),
        reference_scope: "historical_chromosome_record_set".into(),
        engine: "skani".into(),
        engine_version: "0.3.2".into(),
        parameters: "c125_m1000_learnedANI_s80_minAF0_uncapped".into(),
        search_status: "completed".into(),
        adapter_verified: true,
        query_sha256: "a".repeat(64),
        package_sha256: "f".repeat(64),
        diagnostic_exclusion: false,
    }
}

fn groups(spec: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
    spec.iter()
        .map(|(g, ms)| (g.to_string(), ms.iter().map(|m| m.to_string()).collect()))
        .collect()
}

fn definition(spec: &[(&str, &[&str])]) -> Definition {
    Definition::new(DEFINITION_VERSION, groups(spec)).unwrap()
}

/// A member row as JSON, so values reach the production parser as written.
fn hit(id: &str, ani: Value, aq: Value, ar: Value) -> Value {
    json!({"reference_id": id, "ani_percent": ani, "aligned_fraction_query_percent": aq,
        "aligned_fraction_reference_percent": ar})
}

fn h(id: &str, ani: &str) -> Value {
    hit(id, num(ani), json!(80), json!(80))
}

fn num(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn rows(values: &[Value]) -> Result<Vec<MemberRow>, String> {
    values
        .iter()
        .map(|v| serde_json::from_value(v.clone()).map_err(|e| e.to_string()))
        .collect()
}

struct Case {
    version: &'static str,
    groups: Vec<(&'static str, Vec<&'static str>)>,
    rows: Vec<Value>,
    ctx: Context,
}

impl Case {
    /// GV01 = {a, b}, GV02 = {c}; a and b at exactly 99.50, c at 99.49.
    fn new() -> Self {
        Case {
            version: DEFINITION_VERSION,
            groups: vec![("GV01", vec!["a", "b"]), ("GV02", vec!["c"])],
            rows: vec![h("a", "99.50"), h("b", "99.50"), h("c", "99.49")],
            ctx: context(),
        }
    }

    fn definition(&self) -> Definition {
        let spec: Vec<(&str, &[&str])> = self
            .groups
            .iter()
            .map(|(g, ms)| (*g, ms.as_slice()))
            .collect();
        Definition::new(self.version, groups(&spec)).unwrap()
    }

    fn run(&self) -> Value {
        assess(&self.definition(), &rows(&self.rows).unwrap(), &self.ctx)
            .unwrap()
            .to_value()
    }
}

fn reasons(result: &Value) -> Vec<String> {
    result["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn inclusive_boundary_assigns_the_unique_group() {
    let r = Case::new().run();
    assert_eq!(r["genomovar_id"], "GV01");
    assert_eq!(r["status"], "assigned");
    assert_eq!(r["reasons"], json!([]));
}

#[test]
fn a_representative_cannot_override_a_failed_member() {
    let mut case = Case::new();
    case.rows[1] = h("b", "99.49");
    let r = case.run();
    assert_eq!(r["genomovar_id"], Value::Null);
    assert_eq!(r["status"], "withheld");
}

#[test]
fn the_boundary_warning_uses_the_value_not_its_text() {
    let mut case = Case::new();
    case.rows[0] = h("a", "99.5");
    let r = case.run();
    assert!(r["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w.as_str().unwrap().contains("reported inclusive cutoff")));
}

#[test]
fn a_missing_required_member_blocks() {
    let mut case = Case::new();
    case.rows.remove(1);
    assert!(reasons(&case.run()).contains(&"incomplete_alternative_or_member_evidence".into()));
}

#[test]
fn a_missing_alternative_blocks_uniqueness() {
    let mut case = Case::new();
    case.rows.pop();
    let r = case.run();
    assert_eq!(r["compatible_groups"], json!(["GV01"]));
    assert_eq!(r["genomovar_id"], Value::Null);
}

#[test]
fn an_observed_failure_excludes_a_rival_despite_another_missing_member() {
    let mut case = Case::new();
    case.groups[1].1.push("d");
    assert_eq!(case.run()["genomovar_id"], "GV01");
}

#[test]
fn an_overlap_cannot_be_broken_by_the_best_match() {
    let mut case = Case::new();
    case.rows[2] = h("c", "99.99");
    let r = case.run();
    assert!(reasons(&r).contains(&"overlapping_definitions".into()));
    assert_eq!(r["genomovar_id"], Value::Null);
}

#[test]
fn a_low_af_rival_is_still_an_overlap() {
    let mut case = Case::new();
    case.rows[2] = hit("c", num("99.99"), json!(2), json!(3));
    assert!(reasons(&case.run()).contains(&"overlapping_definitions".into()));
}

#[test]
fn each_directional_member_af_is_gated_and_keeps_the_compatible_group() {
    for field in [
        "aligned_fraction_query_percent",
        "aligned_fraction_reference_percent",
    ] {
        let mut case = Case::new();
        case.rows[1][field] = num("79.99");
        let r = case.run();
        assert_eq!(
            reasons(&r),
            vec!["insufficient_member_coverage".to_owned()],
            "{field}"
        );
        assert_eq!(r["genomovar_id"], Value::Null, "{field}");
        // Withholding keeps the ANI-compatible group and the measured coverage failure.
        assert_eq!(r["compatible_groups"], json!(["GV01"]), "{field}");
        assert_eq!(r["groups"][0]["below_member_af"], json!(["b"]), "{field}");
    }
    let mut case = Case::new();
    case.rows[1]["aligned_fraction_query_percent"] = num("80.00");
    assert_eq!(case.run()["genomovar_id"], "GV01");
}

#[test]
fn a_singleton_carries_a_sparse_warning_not_a_population_claim() {
    let mut case = Case::new();
    case.rows = vec![h("a", "99.49"), h("b", "99.50"), h("c", "99.50")];
    let r = case.run();
    assert_eq!(r["genomovar_id"], "GV02");
    assert!(r["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w.as_str().unwrap().contains("Singleton")));
}

#[test]
fn a_self_excluded_singleton_is_unassessable() {
    let mut case = Case::new();
    case.groups[1].1.clear();
    case.rows.pop();
    case.ctx.diagnostic_exclusion = true;
    let r = case.run();
    assert_eq!(r["incomplete_groups"], json!(["GV02"]));
    assert_eq!(r["genomovar_id"], Value::Null);
}

/// The context has no query name or accession, so metadata cannot remove a member.
#[test]
fn accession_metadata_cannot_trigger_self_exclusion() {
    let mut case = Case::new();
    case.rows.remove(1);
    assert_eq!(case.run()["genomovar_id"], Value::Null);
}

#[test]
fn a_candidate_species_only_or_other_parent_is_withheld() {
    type Edit = (&'static str, fn(&mut Parent));
    let edits: Vec<Edit> = vec![
        ("candidate", |p| p.status = "candidate".into()),
        ("unresolved", |p| p.status = "unresolved".into()),
        ("unverified", |p| p.verified = false),
        ("species only", |p| p.subspecies_id = None),
        ("other species", |p| {
            p.species_id = Some("ECTX-SP001".into())
        }),
        ("other subspecies", |p| {
            p.subspecies_id = Some("other".into())
        }),
        ("rescue policy", |p| {
            p.policy_id = "ANIm-species-rescue".into()
        }),
        ("custom policy", |p| {
            p.policy_id = "custom-thresholds".into()
        }),
    ];
    for (name, edit) in edits {
        let mut case = Case::new();
        edit(&mut case.ctx.parent);
        let r = case.run();
        assert!(
            reasons(&r).contains(&"absent_eligible_parent".into()),
            "{name}"
        );
        assert_eq!(r["genomovar_id"], Value::Null, "{name}");
    }
}

#[test]
fn unsupported_scopes_are_withheld() {
    let edits: Vec<fn(&mut Context)> = vec![
        |c| c.input_kind = "reads".into(),
        |c| c.input_kind = "metagenome".into(),
        |c| c.query_scope = "manually_filtered".into(),
        |c| c.reference_scope = "whole_assembly".into(),
    ];
    for edit in edits {
        let mut case = Case::new();
        edit(&mut case.ctx);
        assert!(reasons(&case.run()).contains(&"unsupported_scope".into()));
    }
}

#[test]
fn an_unknown_method_precision_setting_or_definition_is_unsupported() {
    let edits: Vec<fn(&mut Case)> = vec![
        |c| c.ctx.engine = "ANIm".into(),
        |c| c.ctx.engine_version = "0.3.1".into(),
        |c| c.ctx.parameters = "top-five".into(),
        |c| c.version = "new",
    ];
    for edit in edits {
        let mut case = Case::new();
        edit(&mut case);
        assert!(reasons(&case.run()).contains(&"unsupported_method_or_definition".into()));
    }
}

#[test]
fn unbound_or_failed_execution_and_missing_context_fail_closed() {
    let edits: Vec<fn(&mut Context)> = vec![
        |c| c.query_sha256 = "b".repeat(64),
        |c| c.query_sha256 = "a".into(),
        |c| c.package_sha256 = "c".repeat(64),
        |c| c.search_status = "failed".into(),
        |c| c.adapter_verified = false,
    ];
    for edit in edits {
        let mut case = Case::new();
        edit(&mut case.ctx);
        assert!(reasons(&case.run()).contains(&"incomplete_or_unbound_execution".into()));
    }
    let mut case = Case::new();
    case.ctx = Context::default();
    assert_eq!(case.run()["genomovar_id"], Value::Null);
}

#[test]
fn simultaneous_vetoes_are_all_retained() {
    let mut case = Case::new();
    case.ctx.parent.scope_outcome = "unexplained_after_checks".into();
    case.ctx.parent.blocking_reasons = vec!["two_parent_support_alert".into()];
    case.rows[2] = h("c", "99.99");
    let got = reasons(&case.run());
    for wanted in [
        "unresolved_scope",
        "upstream_veto",
        "overlapping_definitions",
    ] {
        assert!(got.contains(&wanted.into()), "{wanted} in {got:?}");
    }
}

/// The context carries no query QC, so CheckM2-style values cannot become a new gate.
#[test]
fn query_qc_does_not_create_a_new_gate() {
    let case = Case::new();
    assert_eq!(case.run()["genomovar_id"], "GV01");
    let rule = &case.run()["rule"];
    assert_eq!(
        rule.as_object().unwrap().keys().collect::<Vec<_>>(),
        vec![
            "ani_minimum_percent",
            "both_af_minimum_percent",
            "every_member",
            "rival_exclusion"
        ]
    );
}

#[test]
fn input_order_does_not_change_the_result() {
    let case = Case::new();
    let mut reversed = Case::new();
    reversed.groups.reverse();
    reversed.rows.reverse();
    assert_eq!(case.run(), reversed.run());
}

#[test]
fn corrupt_values_and_duplicate_evidence_are_rejected() {
    for ani in [
        json!("NaN"),
        json!("Infinity"),
        json!(-1),
        num("100.1"),
        json!(true),
        num("99.499"),
        Value::Null,
    ] {
        let mut case = Case::new();
        case.rows[0]["ani_percent"] = ani.clone();
        assert!(rows(&case.rows).is_err(), "ANI {ani}");
    }
    for af in [
        json!(-1),
        json!(101),
        json!("NaN"),
        json!(true),
        Value::Null,
    ] {
        let mut case = Case::new();
        case.rows[0]["aligned_fraction_query_percent"] = af.clone();
        assert!(rows(&case.rows).is_err(), "AF {af}");
    }
    let case = Case::new();
    let zero_af = rows(&[hit("a", num("99.50"), json!(0), json!(80))]).unwrap();
    assert!(assess(&case.definition(), &zero_af, &case.ctx).is_err());
    for extra in [h("a", "99.50"), h("unknown", "99.50")] {
        let mut values = case.rows.clone();
        values.push(extra);
        assert!(assess(&case.definition(), &rows(&values).unwrap(), &case.ctx).is_err());
    }
    assert!(Definition::new(DEFINITION_VERSION, groups(&[("g", &["a", "a"])])).is_err());
    assert!(Definition::new(DEFINITION_VERSION, groups(&[("g", &["a"]), ("h", &["a"])])).is_err());
}

/// The coordinator review's exhaustive small fixture space: five states for each of four
/// members of two groups, under the approved member-AF gate and the parent-only sensitivity
/// arm, 5^4 * 2 = 1,250 cases. The expected values follow the review's independent predicate.
#[test]
fn all_1250_small_policy_cases_match_the_independent_predicate() {
    let members = ["a", "b", "c", "d"];
    let spec: &[(&str, &[&str])] = &[("G1", &["a", "b"]), ("G2", &["c", "d"])];
    let def = definition(spec);
    let states: [Option<(&str, &str, &str)>; 5] = [
        None,
        Some(("99.49", "90", "90")),
        Some(("99.50", "80", "80")),
        Some(("99.99", "79.99", "90")),
        Some(("99.99", "90", "79.99")),
    ];
    let hundredths = |s: &str| -> u32 {
        let (w, f) = s.split_once('.').unwrap_or((s, ""));
        w.parse::<u32>().unwrap() * 100 + format!("{f:0<2}").parse::<u32>().unwrap()
    };
    let mut cases = 0;
    let mut failures = Vec::new();
    for index in 0..5_usize.pow(4) {
        let combo: Vec<_> = (0..4).map(|i| states[index / 5_usize.pow(i) % 5]).collect();
        let values: Vec<Value> = members
            .iter()
            .zip(&combo)
            .filter_map(|(m, s)| s.map(|(a, q, r)| hit(m, num(a), num(q), num(r))))
            .collect();
        let state = |m: &str| combo[members.iter().position(|x| *x == m).unwrap()];
        let mut compatible = Vec::new();
        let mut unknown = Vec::new();
        for (group, ms) in spec {
            let ss: Vec<_> = ms.iter().map(|m| state(m)).collect();
            if ss.iter().flatten().any(|s| hundredths(s.0) < 9950) {
                continue;
            }
            if ss.iter().any(Option::is_none) {
                unknown.push(*group);
            } else {
                compatible.push(*group);
            }
        }
        let result = assess(&def, &rows(&values).unwrap(), &context())
            .unwrap()
            .to_value();
        let got_reasons = reasons(&result);
        for member_af in [true, false] {
            let mut candidate =
                (compatible.len() == 1 && unknown.is_empty()).then(|| compatible[0]);
            if let Some(group) = candidate.filter(|_| member_af) {
                let ms = spec.iter().find(|(g, _)| *g == group).unwrap().1;
                if ms.iter().any(|m| {
                    let (_, q, r) = state(m).unwrap();
                    hundredths(q).min(hundredths(r)) < 8000
                }) {
                    candidate = None;
                }
            }
            let got = if member_af {
                result["genomovar_id"].as_str().map(str::to_owned)
            } else {
                // Without the member-AF gate, the only remaining reason would be coverage.
                (compatible.len() == 1
                    && got_reasons
                        .iter()
                        .all(|r| r == "insufficient_member_coverage"))
                .then(|| result["compatible_groups"][0].as_str().unwrap().to_owned())
            };
            if got.as_deref() != candidate || result["compatible_groups"] != json!(compatible) {
                failures.push(format!("{combo:?} member_af={member_af}: {result}"));
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 1250);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
