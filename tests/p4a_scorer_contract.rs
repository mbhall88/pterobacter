//! The frozen P4a-r3 reference-scorer fixtures against the two hashed maps P4b uses to read
//! Terobacter output: `docs/contracts/reason-vocabulary.tsv` and `docs/contracts/scorer-fields.tsv`.
//!
//! The scorer belongs to P4b. These tests check that every contract reason and record field it
//! reads has a source in Terobacter output, and that every mapped path exists in real output.
mod common;

use std::collections::BTreeSet;

use common::{p1, reason_vocabulary};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use terobacter::decision::{decide, Policy};
use terobacter::evidence::SavedEvidence;
use terobacter::genomovar::{
    assess, Context, Definition, GenomovarParent, MemberRow, DEFINITION_VERSION, ENGINE,
    ENGINE_VERSION, PARENT_SPECIES_ID, PARENT_SUBSPECIES_ID, QUERY_SCOPE, REFERENCE_SCOPE,
    SEARCH_PARAMETERS,
};

const FIXTURE: &[u8] = include_bytes!("fixtures/p4a-scorer/scoring-cases.json");
const FIXTURE_SHA256: &str = "9a93bf22072bcdf5de7664bf1fc7e34311582491b369fcfdcfd57b33427c5720";
const V1: &str = "conservative-assembly-v1";
const V2: &str = "conservative-assembly-anim-v2";
const GENOMOVAR: &str = "p3a-genomovar-contract-r1";

fn fixture() -> Value {
    serde_json::from_slice(FIXTURE).unwrap()
}

struct FieldRow {
    record: String,
    scorer_field: String,
    supplied_by: String,
    output: String,
    path: String,
}

fn scorer_fields() -> Vec<FieldRow> {
    let text = include_str!("../docs/contracts/scorer-fields.tsv");
    let mut lines = text.lines();
    assert_eq!(
        lines.next(),
        Some("record\tscorer_field\tsupplied_by\tterobacter_output\tterobacter_path\trule")
    );
    lines
        .map(|line| {
            let f: Vec<_> = line.split('\t').collect();
            assert_eq!(f.len(), 6, "malformed scorer-field row {f:?}");
            assert!(
                ["terobacter", "p4b"].contains(&f[2]),
                "unknown supplier in {f:?}"
            );
            FieldRow {
                record: f[0].to_owned(),
                scorer_field: f[1].to_owned(),
                supplied_by: f[2].to_owned(),
                output: f[3].to_owned(),
                path: f[4].to_owned(),
            }
        })
        .collect()
}

/// Every leaf field of a scorer record as a dotted path. A list is a leaf.
fn leaves(value: &Value, prefix: &str, out: &mut BTreeSet<String>) {
    match value.as_object() {
        Some(map) => {
            for (key, child) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                leaves(child, &path, out);
            }
        }
        None => {
            out.insert(prefix.to_owned());
        }
    }
}

fn records<'a>(data: &'a Value, sections: &[&str]) -> Vec<(&'a str, &'a Value)> {
    sections
        .iter()
        .flat_map(|section| data[*section].as_array().unwrap())
        .map(|case| (case["id"].as_str().unwrap(), &case["record"]))
        .collect()
}

fn contract_reasons(policy: &str, field: &str) -> BTreeSet<String> {
    reason_vocabulary(policy)
        .into_iter()
        .filter(|row| row.field == field)
        .map(|row| row.contract_reason)
        .collect()
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}

/// Follow a dotted path through objects. The final key must exist; its value may be null.
fn resolves(value: &Value, path: &str) -> bool {
    let mut current = value;
    for key in path.split('.') {
        match current.as_object().and_then(|map| map.get(key)) {
            Some(next) => current = next,
            None => return false,
        }
    }
    true
}

#[test]
fn fixture_is_the_frozen_p4a_file_and_all_84_cases_are_accounted_for() {
    assert_eq!(format!("{:x}", Sha256::digest(FIXTURE)), FIXTURE_SHA256);
    let data = fixture();
    let count = |section: &str| data[section].as_array().unwrap().len();
    let sections = [
        ("rescue", 40),
        ("rescue_raises", 4),
        ("genomovar", 26),
        ("summary", 4),
        ("clusters", 1),
        ("intervals", 9),
    ];
    for (section, expected) in sections {
        assert_eq!(count(section), expected, "{section}");
    }
    assert_eq!(sections.iter().map(|(s, _)| count(s)).sum::<usize>(), 84);

    // Summary, cluster and interval cases test the scorer's own arithmetic over its categories
    // and the manifest. They read no Terobacter output.
    let allowed: BTreeSet<&str> = ["category", "manifest_state", "cluster"].into();
    for section in ["summary", "clusters"] {
        for case in data[section].as_array().unwrap() {
            for row in case["rows"].as_array().unwrap() {
                for key in row.as_object().unwrap().keys() {
                    assert!(allowed.contains(key.as_str()), "{}: {key}", case["id"]);
                }
            }
        }
    }
    for case in data["intervals"].as_array().unwrap() {
        let keys: BTreeSet<_> = case.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["ci", "k", "n"].map(String::from).into());
    }
}

#[test]
fn every_contract_reason_in_the_scorer_fixtures_has_a_terobacter_reason() {
    let data = fixture();
    let v1 = contract_reasons(V1, "reasons");
    let v2 = contract_reasons(V2, "reasons");
    let genomovar = contract_reasons(GENOMOVAR, "genomovar.reasons");
    let mut unmapped = Vec::new();
    for (id, record) in records(&data, &["rescue", "rescue_raises"]) {
        for (field, known) in [
            (&record["v1"]["reasons"], &v1),
            (&record["v2"]["legacy_v1_result"]["reasons"], &v1),
            (&record["v2"]["reasons"], &v2),
        ] {
            for reason in strings(field) {
                if !known.contains(&reason) {
                    unmapped.push(format!("{id}: {reason}"));
                }
            }
        }
    }
    for (id, record) in records(&data, &["genomovar"]) {
        for reason in strings(&record["gv"]["reasons"]) {
            if !genomovar.contains(&reason) {
                unmapped.push(format!("{id}: {reason}"));
            }
        }
    }
    // Two cases test the scorer itself with names no contract defines: R35 checks that reason
    // order is ignored using the placeholders `a` and `b`, and G20 checks that an unknown
    // genomovar reason is a specification violation. Terobacter never emits either.
    unmapped.sort();
    unmapped.dedup();
    assert_eq!(
        unmapped,
        [
            "G20_unknown_reason: mystery",
            "R35_reason_order_ignored: a",
            "R35_reason_order_ignored: b"
        ]
    );
    let g20 = &data["genomovar"].as_array().unwrap()[19];
    assert_eq!(g20["id"], "G20_unknown_reason");
    assert_eq!(g20["expected"]["category"], "spec_violation");
    assert_eq!(
        g20["expected"]["violations"],
        json!(["unknown_genomovar_reason"])
    );
}

#[test]
fn every_scorer_field_in_the_fixtures_has_a_mapped_source_and_none_is_unused() {
    let data = fixture();
    let fields = scorer_fields();
    let mut used = BTreeSet::new();
    let mut unmapped = Vec::new();
    for (kind, sections) in [
        ("rescue", &["rescue", "rescue_raises"][..]),
        ("genomovar", &["genomovar"][..]),
    ] {
        for (id, record) in records(&data, sections) {
            let mut found = BTreeSet::new();
            leaves(record, "", &mut found);
            for leaf in found {
                let row = fields.iter().find(|row| {
                    row.record == kind
                        && (leaf == row.scorer_field
                            || leaf.starts_with(&format!("{}.", row.scorer_field)))
                });
                match row {
                    Some(row) => {
                        used.insert((kind, row.scorer_field.clone()));
                    }
                    None => unmapped.push(format!("{id}: {leaf}")),
                }
            }
        }
    }
    assert!(unmapped.is_empty(), "unmapped scorer fields: {unmapped:?}");
    let unused: Vec<_> = fields
        .iter()
        .filter(|row| !used.contains(&(row.record.as_str(), row.scorer_field.clone())))
        .map(|row| format!("{}: {}", row.record, row.scorer_field))
        .collect();
    assert!(unused.is_empty(), "mapped but never read: {unused:?}");
}

fn genomovar_context() -> Context {
    let query_sha256 = "a".repeat(64);
    let package_sha256 = "b".repeat(64);
    Context {
        parent: GenomovarParent {
            policy_id: V1.into(),
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

fn genomovar_inputs() -> (Definition, Vec<MemberRow>) {
    let groups = [
        ("GV01".to_owned(), vec!["a".to_owned(), "b".to_owned()]),
        ("GV02".to_owned(), vec!["c".to_owned()]),
    ]
    .into();
    let row = |id: &str, ani: &str| {
        serde_json::from_value(json!({"reference_id": id,
            "ani_percent": ani.parse::<serde_json::Number>().unwrap(),
            "aligned_fraction_query_percent": 90, "aligned_fraction_reference_percent": 90}))
        .unwrap()
    };
    (
        Definition::new(DEFINITION_VERSION, groups).unwrap(),
        vec![row("a", "99.8"), row("b", "99.7"), row("c", "97")],
    )
}

/// One real output of each kind the field map names.
fn outputs() -> Vec<(&'static str, Value)> {
    let case = p1::cases()
        .into_iter()
        .find(|c| c.get("raises").is_none() && c["expected"]["basis"] == "ANIm_assisted")
        .unwrap();
    let v2 = p1::decide(&case).unwrap();
    let evidence = SavedEvidence::from_json(&p1::evidence_text(
        case["id"].as_str().unwrap(),
        &case["input"],
    ))
    .unwrap();
    let v1 = decide(&evidence, &Policy::v1()).to_value().clone();
    let (definition, rows) = genomovar_inputs();
    let section = assess(&definition, &rows, &genomovar_context())
        .unwrap()
        .to_value();
    assert_eq!(section["genomovar_id"], "GV01");
    let (rescue, records) = p1::input(case["id"].as_str().unwrap(), &case["input"], &[]).unwrap();
    let plan = rescue
        .plan(&rescue.bind_supplied(&records, p1::PROFILE).unwrap())
        .unwrap()
        .to_value()
        .clone();
    assert_eq!(plan["trigger_state"], "triggered_with_witness_candidate");
    #[allow(unused_mut)]
    let mut found = vec![
        ("rescue-plan", plan),
        ("v1-report", v1),
        ("v2-report", v2.clone()),
        ("parent-report", v2),
        ("genomovar-section", section),
    ];
    #[cfg(feature = "dev")]
    {
        let diagnostic = terobacter::genomovar::diagnostic_assess(
            &definition,
            &rows,
            &genomovar_context(),
            &["a".to_owned()],
            &[],
            &[],
        )
        .unwrap()
        .to_value();
        found.push(("genomovar-diagnostic", json!({ "diagnostic": diagnostic })));
    }
    found
}

#[test]
fn every_mapped_terobacter_path_exists_in_real_output() {
    let outputs = outputs();
    let mut missing = Vec::new();
    // Only P4b may supply a field with no terobacter path. A P4b-supplied field that names a
    // terobacter source is checked too.
    for row in scorer_fields() {
        if row.path == "-" {
            assert_eq!(
                row.supplied_by, "p4b",
                "{}: no source path",
                row.scorer_field
            );
            continue;
        }
        let Some((_, output)) = outputs.iter().find(|(kind, _)| *kind == row.output) else {
            // The diagnostic output exists only in the development build.
            assert!(
                row.output == "genomovar-diagnostic" && cfg!(not(feature = "dev")),
                "{}: unknown output {}",
                row.scorer_field,
                row.output
            );
            continue;
        };
        if !resolves(output, &row.path) {
            missing.push(format!("{}: {} {}", row.scorer_field, row.output, row.path));
        }
    }
    assert!(missing.is_empty(), "paths missing from output: {missing:?}");
}

/// Reason literals pushed in the genomovar module: `push("reason")` or `push("reason".into())`.
fn emitted_genomovar_reasons() -> BTreeSet<String> {
    let source = include_str!("../src/genomovar.rs");
    source
        .split("push(\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .filter(|literal| {
            !literal.is_empty() && literal.chars().all(|c| c.is_ascii_lowercase() || c == '_')
        })
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_genomovar_map_matches_every_reason_the_module_emits() {
    let emitted = emitted_genomovar_reasons();
    assert!(emitted.contains("absent_eligible_parent"), "{emitted:?}");
    let mapped: BTreeSet<String> = reason_vocabulary(GENOMOVAR)
        .into_iter()
        .map(|row| row.terobacter_reason)
        .collect();
    assert_eq!(emitted, mapped);
}

#[test]
fn every_terobacter_reason_maps_to_exactly_one_contract_reason() {
    for policy in [V1, V2, GENOMOVAR] {
        let rows = reason_vocabulary(policy);
        assert!(!rows.is_empty(), "{policy}");
        let mut seen = BTreeSet::new();
        for row in rows {
            assert!(
                seen.insert((row.field.clone(), row.terobacter_reason.clone())),
                "{policy}: {} {} mapped twice",
                row.field,
                row.terobacter_reason
            );
        }
    }
}

/// The handoff page gives P4b a digest for each map; a map edited without updating the page
/// fails here rather than at binding.
#[test]
fn the_handoff_page_states_the_digest_of_each_map() {
    let page = include_str!("../docs/contracts/evaluation-handoff.md");
    for (name, bytes) in [
        (
            "reason-vocabulary.tsv",
            include_bytes!("../docs/contracts/reason-vocabulary.tsv").as_slice(),
        ),
        (
            "scorer-fields.tsv",
            include_bytes!("../docs/contracts/scorer-fields.tsv").as_slice(),
        ),
    ] {
        let digest = format!("{:x}", Sha256::digest(bytes));
        let row = page
            .lines()
            .find(|line| line.contains(&format!("]({name})")) && line.contains("SHA-256"))
            .unwrap_or_else(|| panic!("no identity row for {name}"));
        assert!(row.contains(&format!("`{digest}`")), "{name}: {row}");
    }
}
