//! Helpers shared by the integration tests.
#![allow(dead_code)] // each test crate uses a different subset

pub mod assembly;
pub mod database;
pub mod mlst;
pub mod native;
pub mod p1;
pub mod package;

use serde_json::{json, Value};
use terobacter::decision::{decide, Policy};
use terobacter::evidence::SavedEvidence;

pub const FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/v1-species-assigned/evidence.json"
);
pub const FIXTURE: &str = include_str!("../fixtures/v1-species-assigned/evidence.json");
const FROZEN_CALL: &str = include_str!("../fixtures/v1-species-assigned/frozen-v1-call.json");

/// Check a decision against the call the frozen 0.2.0 report made for the fixture.
/// The expected values come from the frozen file, never from this implementation.
pub fn assert_matches_frozen_call(decision: &Value) {
    let frozen: Value = serde_json::from_str(FROZEN_CALL).unwrap();

    assert_eq!(decision["policy"]["id"], frozen["policy"]);
    assert_eq!(decision["species"]["status"], frozen["status"]);
    assert_eq!(decision["species"]["reason"], frozen["reason"]);
    assert_eq!(decision["species"]["id"], frozen["species_id"]);
    assert_eq!(decision["species"]["name"], frozen["species_name"]);
}

/// One comparison row for hand-built evidence.
#[derive(Clone, Copy)]
pub struct Row {
    id: &'static str,
    species: Option<&'static str>,
    subspecies: Option<&'static str>,
    lineage: Option<&'static str>,
    ani: f64,
    af_query: f64,
    af_reference: f64,
}

/// A reference of `species` (or an unmapped one, for `None`) with the given ANI and both AFs.
pub fn row(id: &'static str, species: Option<&'static str>, ani: f64, aq: f64, ar: f64) -> Row {
    Row {
        id,
        species,
        lineage: None,
        subspecies: None,
        ani,
        af_query: aq,
        af_reference: ar,
    }
}

impl Row {
    pub fn subspecies(self, subspecies: &'static str) -> Row {
        Row {
            subspecies: Some(subspecies),
            ..self
        }
    }

    pub fn lineage(self, lineage: &'static str) -> Row {
        Row {
            lineage: Some(lineage),
            ..self
        }
    }
}

fn hundredths(percent: f64) -> i64 {
    (percent * 100.0).round() as i64
}

/// Valid saved evidence for `rows`, which must already be in rank order (highest ANI first,
/// then reference ID). Counts and nearest IDs are filled in to match.
pub fn evidence_json(rows: &[Row]) -> Value {
    let rank = |r: &Row| (-hundredths(r.ani), r.id);
    assert!(
        rows.windows(2).all(|w| rank(&w[0]) < rank(&w[1])),
        "rows must be in rank order"
    );
    let nearest: Vec<_> = rows
        .iter()
        .take_while(|r| hundredths(r.ani) == hundredths(rows[0].ani))
        .map(|r| r.id)
        .collect();
    let references: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "reference_id": r.id,
                "species_id": r.species.unwrap_or(""),
                "species_name": r.species.map_or(String::new(), |s| format!("Enterobacter {s}")),
                "lineage_id": r.lineage.unwrap_or(""),
                "subspecies_id": r.subspecies.unwrap_or(""),
                "subspecies_name": r.subspecies.map_or(String::new(), |s| format!("Enterobacter {s}")),
                "reference_role": if r.species.is_none() && r.lineage.is_none() { "context_only" } else { "diversity" },
                "name_status": "validly_published_correct_name",
                "ani_percent": r.ani,
                "aligned_fraction_query_percent": r.af_query,
                "aligned_fraction_reference_percent": r.af_reference,
                "minimum_directional_af_percent": r.af_query.min(r.af_reference),
            })
        })
        .collect();
    json!({
        "schema_version": "2.0.0-dev-evidence",
        "record_type": "development_evidence_only",
        "input_kind": "assembly",
        "units": {"ani": "percent", "aligned_fraction": "percent"},
        "assignment": {"status": "not_evaluated", "reason": "development_evidence_only",
            "species_id": null, "species_name": null, "subspecies_id": null,
            "subspecies_name": null, "lineage_id": null, "genomovar_id": null},
        "provenance": {
            "input_kind": "assembly",
            "engine_version": "0.3.2",
            "sequence_scope": "whole_assembly_as_supplied",
            "reference_component_id": "primary_whole",
            "reference_package_sha256": "a".repeat(64),
            "query_sha256": "b".repeat(64),
            "search_exit_code": 0,
        },
        "evidence_status": "estimates_returned",
        "reference_count": rows.len(),
        "raw_hit_count": rows.len(),
        "unique_hit_count": rows.len(),
        "not_returned_reference_count": 0,
        "duplicate_rows_collapsed": 0,
        "nearest_reference_ids": nearest,
        "ranked_reference_evidence": references,
    })
}

/// Evidence text for `rows` in which each `(field, literal)` of the reference at `index` is
/// written as `literal`, exactly as given. Building such a number through a `Value` would pass it
/// through an f64 first and hide any rounding the parser does.
pub fn evidence_text_with_literals(
    rows: &[Row],
    index: usize,
    overrides: &[(&str, &str)],
) -> String {
    let mut evidence = evidence_json(rows);
    for (n, (field, _)) in overrides.iter().enumerate() {
        evidence["ranked_reference_evidence"][index][*field] = format!("@literal-{n}@").into();
    }
    let mut text = evidence.to_string();
    for (n, (_, literal)) in overrides.iter().enumerate() {
        text = text.replace(&format!("\"@literal-{n}@\""), literal);
    }
    text
}

/// One row of the hashed reason-vocabulary map in `docs/contracts/reason-vocabulary.tsv`.
#[allow(dead_code)]
pub struct VocabularyRow {
    pub field: String,
    pub terobacter_reason: String,
    pub contract_reason: String,
}

/// The reason-vocabulary rows for one policy or contract, in file order.
#[allow(dead_code)]
pub fn reason_vocabulary(policy: &str) -> Vec<VocabularyRow> {
    let text = include_str!("../../docs/contracts/reason-vocabulary.tsv");
    let mut lines = text.lines();
    assert_eq!(
        lines.next(),
        Some("policy\tfield\tterobacter_reason\tcontract_reason\tcontract_source")
    );
    lines
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .inspect(|f| assert_eq!(f.len(), 5, "malformed vocabulary row {f:?}"))
        .filter(|f| f[0] == policy)
        .map(|f| VocabularyRow {
            field: f[1].to_owned(),
            terobacter_reason: f[2].to_owned(),
            contract_reason: f[3].to_owned(),
        })
        .collect()
}

/// The decision JSON for evidence text that is expected to load.
pub fn decision_from_text(evidence: &str) -> Value {
    let evidence = SavedEvidence::from_json(evidence).expect("evidence should load");
    let decision: Value = serde_json::from_str(&decide(&evidence, &Policy::v1()).to_json())
        .expect("decision is JSON");
    let vocabulary: Vec<String> = reason_vocabulary("conservative-assembly-v1")
        .into_iter()
        .map(|row| row.terobacter_reason)
        .collect();
    for reason in decision["blocking_reasons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_str().unwrap())
        .chain(
            decision["assignment"]["reason"]
                .as_str()
                .unwrap()
                .split(';'),
        )
        .chain(
            ["species", "subspecies"]
                .map(|rank| decision["rank_evidence"][rank]["reason"].as_str().unwrap()),
        )
    {
        assert!(
            vocabulary.iter().any(|known| known == reason),
            "unmapped reason: {reason}"
        );
    }
    decision
}

pub fn decision_for(rows: &[Row]) -> Value {
    decision_from_text(&evidence_json(rows).to_string())
}

/// Saved evidence over every primary reference in the committed fixture, bound to
/// `package_sha256`. Each row says what the package says about its reference, as the research
/// reporter writes it: the species and subspecies concepts with their display names, the name
/// status of the most specific of them, and the reference's role. Rows are ranked in reference
/// order with falling ANI.
pub fn packaged_evidence(package_sha256: &str) -> Value {
    let package = package::Synthetic::new()
        .verify()
        .expect("the fixture verifies");
    let component = package
        .component("primary_whole")
        .expect("a packaged component");
    let concept = |id: &str| package.concept(id).expect("a packaged concept");
    let references: Vec<Value> = component
        .references
        .iter()
        .enumerate()
        .map(|(rank, reference)| {
            let species = concept(&reference.species_id);
            let subspecies = reference.subspecies_id.as_deref().map(concept);
            let ani = 99.0 - rank as f64 / 10.0;
            json!({
                "reference_id": reference.reference_id,
                "species_id": species.id,
                "species_name": species.display_name,
                "lineage_id": "",
                "subspecies_id": subspecies.map_or("", |c| c.id.as_str()),
                "subspecies_name": subspecies.map_or("", |c| c.display_name.as_str()),
                "reference_role": if reference.type_anchor { "type_anchor" } else { "diversity" },
                "name_status": subspecies.unwrap_or(species).nomenclature_status,
                "ani_percent": ani,
                "aligned_fraction_query_percent": 95.0,
                "aligned_fraction_reference_percent": 96.0,
                "minimum_directional_af_percent": 95.0,
            })
        })
        .collect();
    let mut evidence = evidence_json(&[]);
    evidence["provenance"]["reference_package_sha256"] = package_sha256.into();
    evidence["reference_count"] = references.len().into();
    evidence["raw_hit_count"] = references.len().into();
    evidence["unique_hit_count"] = references.len().into();
    evidence["nearest_reference_ids"] = json!([references[0]["reference_id"]]);
    evidence["ranked_reference_evidence"] = references.into();
    evidence
}

/// Stable synthetic DNA generation shared by measurement fixtures.
pub fn dna(length: usize, mut state: u64) -> Vec<u8> {
    (0..length)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            b"ACGT"[(state & 3) as usize]
        })
        .collect()
}
/// Stop a synthetic CLI at an observable stage so a wall deadline expires deterministically.
pub fn hold_at_artifact(child: &mut std::process::Child, artifact: &std::path::Path) {
    use std::time::{Duration, Instant};
    let start = Instant::now();
    while !artifact.exists() {
        if let Some(status) = child.try_wait().unwrap() {
            panic!("process exited before stage {artifact:?}: {status}");
        }
        if start.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("stage not reached: {artifact:?}");
        }
        std::thread::sleep(Duration::from_micros(100));
    }
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGSTOP) }, 0);
    std::thread::sleep(Duration::from_secs(6));
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGCONT) }, 0);
}
