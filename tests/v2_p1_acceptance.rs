//! The 60 P1-r1 acceptance cases, run through the production saved-evidence path.
//!
//! Expected values come from the accepted fixture file, never from this implementation. As in
//! the P1 oracle, `reasons` is a subset check and every other expected field must match exactly.
mod common;

use common::p1;
use serde_json::{json, Value};

/// Where each P1 result field lives in the v2 decision.
fn field(decision: &Value, key: &str) -> Value {
    match key {
        "species" => decision["species_id"].clone(),
        "subspecies" => decision["subspecies_id"].clone(),
        "basis" => decision["call_basis"].clone(),
        "mode" => decision["policy_mode"].clone(),
        "display" => json!(decision["top_references"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["reference_id"].clone())
            .collect::<Vec<_>>()),
        "trigger" | "reasons" | "challenges" | "validation_status" | "competitor_evaluation" => {
            decision[key].clone()
        }
        other => panic!("unmapped P1 field {other}"),
    }
}

#[test]
fn every_p1_acceptance_case_passes_against_the_production_decision() {
    let cases = p1::cases();
    assert_eq!(cases.len(), 60);
    let mut failures = Vec::new();
    for case in &cases {
        let id = case["id"].as_str().unwrap();
        let result = p1::decide(case);
        if case.get("raises").is_some() {
            if let Ok(decision) = result {
                failures.push(format!("{id}: expected rejection, got {decision}"));
            }
            continue;
        }
        let decision = match result {
            Ok(decision) => decision,
            Err(error) => {
                failures.push(format!("{id}: rejected: {error}"));
                continue;
            }
        };
        for (key, expected) in case["expected"].as_object().unwrap() {
            let got = field(&decision, key);
            let ok = if key == "reasons" {
                let got = got.as_array().cloned().unwrap_or_default();
                expected.as_array().unwrap().iter().all(|r| got.contains(r))
            } else {
                got == *expected
            };
            if !ok {
                failures.push(format!("{id}: {key} expected {expected}, got {got}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_fixture_is_the_accepted_p1_file() {
    use sha2::{Digest, Sha256};
    let bytes = include_bytes!("fixtures/p1-acceptance/cases.json");
    assert_eq!(
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        "8b90bdf7c257dd52558a4218e748bd5cf9cb407817997541da4208a1f35ad0bc"
    );
}
