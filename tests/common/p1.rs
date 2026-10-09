//! Turns a P1 acceptance case into the production inputs: 0.2.0 saved evidence, a primary
//! inventory, bound problem warnings and a scope assessment, and integer ANIm pair records.
//!
//! The cases give ANIm as decimal percentages. Each is written as exact integers (D, E, covered
//! bases and lengths) whose rational value equals the decimal, so no case is rounded on the way in.

use serde_json::{json, Number, Value};
use sha2::{Digest, Sha256};
use std::str::FromStr;
use terobacter::evidence::{EvidenceError, SavedEvidence};
use terobacter::panel::Panel;
use terobacter::rescue::{AnimRecords, RescueInput, Thresholds};

pub const PROFILE: &str = "synthetic-anim-profile";
pub const QUERY_SHA256: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
pub const PACKAGE_SHA256: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
/// Every synthetic sequence has this many bases, so a coverage with up to ten decimals is exact.
const LENGTH: u64 = 1_000_000_000_000;

pub fn cases() -> Vec<Value> {
    serde_json::from_str(include_str!("../fixtures/p1-acceptance/cases.json")).unwrap()
}

pub fn sha256(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

/// skani emits two decimals, and saved evidence with more is rejected rather than rounded. One
/// case uses `94.49999` to mean "just below the 94.5 trigger"; the nearest value skani can emit
/// below the trigger is `94.49`. `v2_trigger.rs` checks that the literal itself is rejected.
pub fn skani_emitted(case: &str, reference: &str, value: &str) -> String {
    match (case, reference, value) {
        ("below_trigger_evidence_only", "A1", "94.49999") => "94.49".into(),
        _ => {
            assert!(
                value.split_once('.').is_none_or(|(_, f)| f.len() <= 2),
                "{case}/{reference}: unexpected skani precision {value}"
            );
            value.into()
        }
    }
}

fn number(text: &str) -> Value {
    Value::Number(Number::from_str(text).unwrap())
}

/// Exact integer `value * 10^scale` for a nonnegative decimal string.
fn scaled(value: &str, scale: u32) -> u64 {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    assert!(
        fraction.len() as u32 <= scale,
        "{value} needs more precision"
    );
    let digits = format!("{whole}{fraction:0<width$}", width = scale as usize);
    digits.parse().unwrap()
}

fn text_of(value: &Value) -> Option<&str> {
    value.as_str().filter(|s| !s.is_empty())
}

fn ranked<'a>(case: &str, input: &'a Value) -> Vec<(&'a Value, String)> {
    let mut rows: Vec<_> = input["references"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| {
            text_of(&r["ani"]).map(|ani| (r, skani_emitted(case, r["id"].as_str().unwrap(), ani)))
        })
        .collect();
    rows.sort_by(|(a, x), (b, y)| {
        scaled(y, 2)
            .cmp(&scaled(x, 2))
            .then_with(|| a["id"].as_str().cmp(&b["id"].as_str()))
    });
    rows
}

fn role(r: &Value) -> &'static str {
    if text_of(&r["species"]).is_some() || text_of(&r["lineage"]).is_some() {
        "diversity"
    } else {
        "context_only"
    }
}

fn name(id: Option<&str>) -> String {
    id.map_or(String::new(), |id| format!("Synthetic {id}"))
}

pub fn evidence_text(case: &str, input: &Value) -> String {
    let rows = ranked(case, input);
    let total = input["references"].as_array().unwrap().len();
    let nearest: Vec<_> = rows
        .iter()
        .take_while(|(_, ani)| scaled(ani, 2) == scaled(&rows[0].1, 2))
        .map(|(r, _)| r["id"].clone())
        .collect();
    let evidence: Vec<_> = rows
        .iter()
        .map(|(r, ani)| {
            let (aq, ar) = (r["aq"].as_str().unwrap(), r["ar"].as_str().unwrap());
            let min = if scaled(aq, 2) <= scaled(ar, 2) {
                aq
            } else {
                ar
            };
            json!({"reference_id": r["id"], "species_id": text_of(&r["species"]).unwrap_or(""),
                "species_name": name(text_of(&r["species"])),
                "subspecies_id": text_of(&r["subspecies"]).unwrap_or(""),
                "subspecies_name": name(text_of(&r["subspecies"])),
                "lineage_id": text_of(&r["lineage"]).unwrap_or(""), "reference_role": role(r),
                "name_status": "synthetic", "ani_percent": number(ani),
                "aligned_fraction_query_percent": number(aq),
                "aligned_fraction_reference_percent": number(ar),
                "minimum_directional_af_percent": number(min)})
        })
        .collect();
    let mut out = json!({"schema_version": "2.0.0-dev-evidence", "record_type": "development_evidence_only",
        "input_kind": "assembly", "units": {"ani": "percent", "aligned_fraction": "percent"},
        "assignment": {"status": "not_evaluated", "reason": "development_evidence_only",
            "species_id": null, "species_name": null, "subspecies_id": null,
            "subspecies_name": null, "lineage_id": null, "genomovar_id": null},
        "provenance": {"input_kind": "assembly", "engine_version": "0.3.2",
            "sequence_scope": "whole_assembly_as_supplied", "reference_component_id": "primary_whole",
            "reference_package_sha256": PACKAGE_SHA256, "query_sha256": QUERY_SHA256, "search_exit_code": 0},
        "evidence_status": if rows.is_empty() { "no_estimate_returned" } else { "estimates_returned" },
        "reference_count": total, "raw_hit_count": rows.len(), "unique_hit_count": rows.len(),
        "not_returned_reference_count": total - rows.len(), "duplicate_rows_collapsed": 0,
        "nearest_reference_ids": nearest, "ranked_reference_evidence": evidence});
    if let Some(outcome) = input["scope"].as_str() {
        let checks = if outcome == "not_assessed" {
            json!([])
        } else {
            json!([{"check_id": "scope-1", "method": "synthetic-check", "method_version": "1",
                "artifact_path": "/synthetic/scope.txt", "sha256": "d".repeat(64), "status": "completed"}])
        };
        out["scope_assessment"] = json!({"schema_version": "1.0.0-scope-assessment",
            "query_sha256": QUERY_SHA256, "evidence_sha256": "c".repeat(64), "outcome": outcome,
            "reason": "Synthetic scope assessment", "checks": checks});
    }
    out.to_string()
}

pub fn inventory_text(input: &Value) -> String {
    let references: Vec<_> = input["references"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            json!({"reference_id": r["id"], "source_sha256": sha256(r["id"].as_str().unwrap()),
                "species_id": text_of(&r["species"]).unwrap_or(""), "species_name": name(text_of(&r["species"])),
                "subspecies_id": text_of(&r["subspecies"]).unwrap_or(""),
                "subspecies_name": name(text_of(&r["subspecies"])),
                "lineage_id": text_of(&r["lineage"]).unwrap_or(""), "name_status": "synthetic",
                "reference_role": role(r)})
        })
        .collect();
    json!({"schema_version": "1.0-shadow-primary-inventory", "package_sha256": PACKAGE_SHA256,
        "component_id": "primary_whole", "representation": "synthetic", "references": references})
    .to_string()
}

pub fn warnings_text(evidence: &SavedEvidence, ids: &[&str]) -> String {
    let warnings: Vec<_> = ids
        .iter()
        .map(|id| {
            json!({"warning_id": id, "reason": "Synthetic concern", "source": "synthetic-test",
                "source_version": "1", "artifacts": [{"path": "/synthetic/warning.txt", "sha256": "e".repeat(64)}]})
        })
        .collect();
    json!({"schema_version": "1.0-shadow-problem-warnings", "query_sha256": QUERY_SHA256,
        "reference_package_sha256": PACKAGE_SHA256, "evidence_sha256": evidence.source_sha256(),
        "warnings": warnings})
    .to_string()
}

/// One ANIm orientation as an integer record in biological roles. `QR` has the reference as
/// its native reference; `RQ` swaps the native axes.
pub fn orientation(
    direction: &str,
    reference: &str,
    status: &str,
    ani: &str,
    aq: &str,
    ar: &str,
) -> Value {
    let (native_reference, native_query) = match direction {
        "QR" => (reference, "query"),
        _ => ("query", reference),
    };
    let mut out = orientation_metrics(status, ani, aq, ar);
    out["native_reference_id"] = native_reference.into();
    out["native_query_id"] = native_query.into();
    out
}

fn orientation_metrics(status: &str, ani: &str, aq: &str, ar: &str) -> Value {
    match status {
        "complete" => {
            // identity = 100 (D - 2E) / D = 100 (LENGTH - E) / LENGTH with D = 2 * LENGTH, and
            // LENGTH = 10^12, so E = LENGTH - ani * 10^10 and covered bases = AF * 10^10.
            let d = 2 * LENGTH;
            let e = LENGTH - scaled(ani, 10);
            json!({"method_profile": PROFILE, "status": "aligned", "error": null,
                "metrics": {"status": "aligned", "D": d, "E": e, "blocks": 1,
                    "query_covered": scaled(aq, 10), "query_length": LENGTH,
                    "reference_covered": scaled(ar, 10), "reference_length": LENGTH}})
        }
        "no_alignment" => {
            json!({"method_profile": PROFILE, "status": "no_alignment", "error": null,
            "metrics": {"status": "no_alignment", "D": 0, "E": 0, "blocks": 0,
                "query_covered": 0, "query_length": LENGTH, "reference_covered": 0, "reference_length": LENGTH}})
        }
        _ => json!({"method_profile": PROFILE, "status": "error", "metrics": null,
            "error": "synthetic native failure"}),
    }
}

/// A pair record; `orientations` is raw JSON text so a duplicated key reaches the parser intact.
pub fn pair_record(reference: &str, finished: bool, orientations: &str) -> String {
    let record = json!({"schema": "terobacter-anim-pair-v1",
        "status": if finished { "complete" } else { "incomplete" },
        "query": {"id": "query", "sha256": QUERY_SHA256, "total_bases": LENGTH,
            "scope": "whole_assembly", "component": "query-whole-assembly"},
        "reference": {"id": reference, "sha256": sha256(reference), "total_bases": LENGTH,
            "scope": "whole_assembly", "component": "primary_whole"},
        "method": {"profile": PROFILE}, "orientations": "@orientations@", "error": null});
    record
        .to_string()
        .replace("\"@orientations@\"", orientations)
}

pub fn records_text(input: &Value) -> String {
    let mut lines = Vec::new();
    for (reference, item) in input["anim"].as_object().into_iter().flatten() {
        let directions = item["directions"].as_array().unwrap();
        let finished = directions.len() == 2 && directions.iter().all(|d| d["status"] != "error");
        let parts: Vec<_> = directions
            .iter()
            .map(|d| {
                let o = orientation(
                    d["id"].as_str().unwrap(),
                    reference,
                    d["status"].as_str().unwrap(),
                    d["ani"].as_str().unwrap_or("0"),
                    d["aq"].as_str().unwrap_or("0"),
                    d["ar"].as_str().unwrap_or("0"),
                );
                format!("\"{}\":{o}", d["id"].as_str().unwrap())
            })
            .collect();
        lines.push(pair_record(
            reference,
            finished,
            &format!("{{{}}}", parts.join(",")),
        ));
    }
    lines.join("\n")
}

/// Build the production inputs for a P1-shaped case, masking `mask`.
pub fn input(
    id: &str,
    input: &Value,
    mask: &[&str],
) -> Result<(RescueInput, AnimRecords), EvidenceError> {
    let evidence = SavedEvidence::from_json(&evidence_text(id, input))?;
    let warning_ids: Vec<&str> = input["warnings"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|w| w.as_str().unwrap())
        .collect();
    let warnings = (!warning_ids.is_empty()).then(|| warnings_text(&evidence, &warning_ids));
    let evidence = evidence.with_annotations(None, warnings.as_deref(), None)?;
    let panel = Panel::from_json(&inventory_text(input), &evidence)?;
    let thresholds = match input.get("custom") {
        Some(custom) => Thresholds::from_json(&custom.to_string())?,
        None => Thresholds::standard(),
    };
    let mask: Vec<String> = mask.iter().map(|m| m.to_string()).collect();
    let rescue = RescueInput::new(evidence, panel, &mask, thresholds)?;
    let records = AnimRecords::from_jsonl(&records_text(input))?;
    Ok((rescue, records))
}

/// Run a case through the production saved-evidence path and return the v2 decision JSON,
/// with every provenance-bound ANIm record supplied, as P1 treats supplied evidence.
pub fn decide(case: &Value) -> Result<Value, EvidenceError> {
    let (rescue, records) = input(case["id"].as_str().unwrap(), &case["input"], &[])?;
    let anim = rescue.bind_supplied(&records, PROFILE)?;
    Ok(rescue.decide(&anim).to_value().clone())
}

/// The P1 case called `id`.
pub fn case(id: &str) -> Value {
    cases().into_iter().find(|c| c["id"] == id).unwrap()
}

/// One returned skani row in P1 case shape.
pub fn hit(id: &str, species: Option<&str>, ani: &str, aq: &str, ar: &str) -> Value {
    json!({"id": id, "species": species, "subspecies": null, "ani": ani, "aq": aq, "ar": ar})
}

/// A reference skani did not return.
pub fn unreturned(id: &str, species: Option<&str>) -> Value {
    json!({"id": id, "species": species, "subspecies": null, "ani": null})
}

/// Both orientations with the same identity and AFs.
pub fn reciprocal(ani: &str, aq: &str, ar: &str) -> Value {
    json!({"directions": [
        {"id": "QR", "status": "complete", "ani": ani, "aq": aq, "ar": ar},
        {"id": "RQ", "status": "complete", "ani": ani, "aq": aq, "ar": ar}]})
}

/// A pair whose orientations have the given statuses (`complete` at 90/85/85 otherwise).
pub fn statuses(qr: &str, rq: &str) -> Value {
    let d = |id: &str, s: &str| json!({"id": id, "status": s, "ani": "90", "aq": "85", "ar": "85"});
    json!({"directions": [d("QR", qr), d("RQ", rq)]})
}
