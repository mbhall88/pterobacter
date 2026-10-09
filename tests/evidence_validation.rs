//! Decision seam: saved evidence that could silently change a decision is rejected.

mod common;

use common::{evidence_json, evidence_text_with_literals, row};
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

/// Each literal has more than two decimals and sits just beside a calling threshold. Read through
/// an f64 it would round onto the threshold and decide, so these are written into the JSON text
/// as given rather than built from numbers.
#[test]
fn literals_with_more_than_two_decimals_are_rejected_even_beside_a_threshold() {
    let rows = [
        row("ref-a", Some("SP1"), 96.0, 80.0, 80.0),
        row("ref-b", Some("SP2"), 90.0, 90.0, 90.0),
    ];
    let cases = [
        ("ani_percent", "95.999999999999999"),
        ("ani_percent", "96.000000000000001"),
        ("aligned_fraction_query_percent", "79.999999999999999"),
        ("aligned_fraction_query_percent", "80.000000000000001"),
        ("aligned_fraction_reference_percent", "79.999999999999999"),
        ("aligned_fraction_reference_percent", "80.000000000000001"),
    ];

    for (field, literal) in cases {
        let text = evidence_text_with_literals(&rows, 0, &[(field, literal)]);

        let error = SavedEvidence::from_json(&text)
            .expect_err(&format!("{field} = {literal} should be rejected"));

        assert!(
            error.to_string().contains("two decimals"),
            "{field} = {literal}: {error}"
        );
    }
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

#[test]
fn historical_unresolved_labels_are_rejected() {
    for field in [
        "species_id",
        "species_name",
        "subspecies_id",
        "subspecies_name",
        "lineage_id",
        "genomovar_id",
    ] {
        rejects("empty taxon fields", |e| {
            e["assignment"]["status"] = "unresolved".into();
            e["assignment"][field] = "retained-label".into();
        });
    }
}
#[test]
fn both_query_digest_layouts_work_but_conflicting_digests_are_rejected() {
    let mut e = valid();
    e["provenance"]
        .as_object_mut()
        .unwrap()
        .remove("query_sha256");
    e["provenance"]["sha256"] = serde_json::json!({"query":"b".repeat(64)});
    SavedEvidence::from_json(&e.to_string()).unwrap();
    e["provenance"]["query_sha256"] = "c".repeat(64).into();
    assert!(SavedEvidence::from_json(&e.to_string()).is_err());
}
#[test]
fn a_different_engine_or_sequence_scope_cannot_use_the_frozen_policy() {
    rejects("skani 0.3.2", |e| {
        e["provenance"]["engine_version"] = "0.3.3".into()
    });
    rejects("whole-assembly", |e| {
        e["provenance"]["sequence_scope"] = "chromosome_only".into()
    });
}
#[test]
fn subspecies_metadata_cannot_invent_a_parent_or_reuse_a_species_id() {
    rejects("parent species", |e| {
        let r = &mut e["ranked_reference_evidence"][0];
        r["species_id"] = "".into();
        r["species_name"] = "".into();
        r["reference_role"] = "context_only".into();
        r["subspecies_id"] = "SS1".into();
        r["subspecies_name"] = "subspecies one".into();
    });
    rejects("multiple ranks", |e| {
        let r = &mut e["ranked_reference_evidence"][0];
        r["subspecies_id"] = "SP1".into();
        r["subspecies_name"] = "subspecies one".into();
    });
}

#[test]
fn unmapped_references_must_be_context_only_before_they_can_supply_a_contrast() {
    rejects("context_only", |e| {
        let r = &mut e["ranked_reference_evidence"][1];
        r["species_id"] = "".into();
        r["species_name"] = "".into();
    });
}
