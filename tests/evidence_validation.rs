//! Decision seam: saved evidence that could silently change a decision is rejected.

mod common;

use common::{evidence_json, row};
use serde_json::Value;
use terobacter::evidence::SavedEvidence;

fn valid() -> Value {
    evidence_json(&[
        row("ref-a", Some("SP1"), 99.0, 90.0, 90.0),
        row("ref-b", Some("SP2"), 90.0, 90.0, 90.0),
    ])
}

/// Apply `damage` to valid evidence and require the load to fail with `expected` in the message.
fn rejects(expected: &str, damage: impl FnOnce(&mut Value)) {
    let mut evidence = valid();
    damage(&mut evidence);

    let error =
        SavedEvidence::from_json(&evidence.to_string()).expect_err("evidence should be rejected");

    assert!(
        error.to_string().contains(expected),
        "expected {expected:?} in {error}"
    );
}

#[test]
fn valid_evidence_loads() {
    SavedEvidence::from_json(&valid().to_string()).expect("the baseline must load");
}

#[test]
fn another_schema_is_rejected() {
    rejects("expected schema", |e| e["schema_version"] = "1.0.0".into());
}

#[test]
fn a_failed_search_is_rejected() {
    rejects("successful search", |e| {
        e["provenance"]["search_exit_code"] = 1.into()
    });
}

#[test]
fn a_package_or_query_without_a_sha256_digest_is_rejected() {
    rejects("SHA-256", |e| {
        e["provenance"]["query_sha256"] = "not-a-digest".into()
    });
    rejects("SHA-256", |e| {
        e["provenance"]["reference_package_sha256"] = "A".repeat(64).into()
    });
}

#[test]
fn rows_that_are_not_in_rank_order_are_rejected() {
    rejects("ranked", |e| {
        e["ranked_reference_evidence"]
            .as_array_mut()
            .unwrap()
            .reverse()
    });
}

#[test]
fn a_truncated_panel_is_rejected() {
    rejects("truncated", |e| {
        e["ranked_reference_evidence"].as_array_mut().unwrap().pop();
    });
}

#[test]
fn nearest_ids_that_do_not_match_the_top_rows_are_rejected() {
    rejects("nearest", |e| {
        e["nearest_reference_ids"] = serde_json::json!(["ref-b"])
    });
}

#[test]
fn more_than_two_decimals_are_rejected_not_rounded() {
    rejects("two decimals", |e| {
        e["ranked_reference_evidence"][0]["ani_percent"] = 95.999.into();
    });
}

#[test]
fn a_species_id_without_a_name_is_rejected() {
    rejects("both be present or absent", |e| {
        e["ranked_reference_evidence"][0]["species_name"] = "".into();
    });
}

#[test]
fn one_species_id_with_two_names_is_rejected() {
    rejects("conflicting names", |e| {
        e["ranked_reference_evidence"][1]["species_id"] = "SP1".into();
    });
}

#[test]
fn a_repeated_reference_id_is_rejected() {
    rejects("unique", |e| {
        e["ranked_reference_evidence"][1]["reference_id"] = "ref-a".into();
    });
}
