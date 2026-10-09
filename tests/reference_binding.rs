//! The decision core accepts evidence only when it is bound to the package it is given, and it
//! records that package's identity in the decision.

mod common;

use common::package::{Synthetic, DISCLAIMER, PACKAGE_VERSION};
use common::{evidence_json, packaged_evidence, row, Row};
use serde_json::Value;
use terobacter::decision::{decide, decide_bound, Policy};
use terobacter::evidence::SavedEvidence;
use terobacter::reference::PackageErrorKind;

fn saved(json: &Value) -> SavedEvidence {
    SavedEvidence::from_json(&json.to_string()).expect("evidence loads")
}

fn evidence_over(rows: &[Row], package_sha256: &str) -> SavedEvidence {
    let mut json = evidence_json(rows);
    json["provenance"]["reference_package_sha256"] = package_sha256.into();
    saved(&json)
}

#[test]
fn evidence_bound_to_the_verified_package_records_its_identity_and_components() {
    let package = Synthetic::new().verify().expect("package verifies");
    let digest = package.package().manifest_sha256.clone();
    let evidence = saved(&packaged_evidence(&digest));

    let decision: Value = serde_json::from_str(
        &decide_bound(&evidence, &package, &Policy::v1())
            .expect("evidence is bound")
            .to_json(),
    )
    .unwrap();

    let report = &decision["reference_package"];
    assert_eq!(report["status"], "verified");
    assert_eq!(report["package"]["version"], PACKAGE_VERSION);
    assert_eq!(report["package"]["manifest_sha256"], digest);
    assert_eq!(
        report["licence"]["ncbi_submitter_rights_disclaimer"],
        DISCLAIMER
    );
    assert_eq!(decision["input"]["reference_package_sha256"], digest);
    for id in ["primary_whole", "genomovar_chromosome"] {
        let index = &report["components"][id]["databases"]["index.db"];
        assert_eq!(index["sha256"].as_str().map(str::len), Some(64), "{id}");
    }
}

#[test]
fn evidence_bound_to_another_package_is_unresolved() {
    let package = Synthetic::new().verify().expect("package verifies");
    let evidence = saved(&packaged_evidence(&"c".repeat(64)));

    let error = decide_bound(&evidence, &package, &Policy::v1()).expect_err("not bound");

    assert_eq!(error.kind(), PackageErrorKind::Unresolved, "{error}");
    assert!(
        error.to_string().contains("another reference package"),
        "{error}"
    );
}

#[test]
fn evidence_naming_a_reference_the_package_lacks_is_unresolved() {
    let package = Synthetic::new().verify().expect("package verifies");
    let digest = package.package().manifest_sha256.clone();
    let evidence = evidence_over(
        &[row(
            "primary_whole:GCA_900000009",
            Some("ECTX-SP-alpha"),
            98.5,
            95.0,
            96.0,
        )],
        &digest,
    );

    let error = decide_bound(&evidence, &package, &Policy::v1()).expect_err("unknown reference");

    assert_eq!(error.kind(), PackageErrorKind::Unresolved, "{error}");
    assert!(
        error.to_string().contains("not a reference of the package"),
        "{error}"
    );
}

#[test]
fn evidence_assigning_a_reference_to_other_taxa_is_unresolved() {
    let package = Synthetic::new().verify().expect("package verifies");
    let digest = package.package().manifest_sha256.clone();
    let evidence = evidence_over(
        &[row(
            "primary_whole:GCA_900000002",
            Some("ECTX-SP-gamma"),
            98.5,
            95.0,
            96.0,
        )
        .subspecies("ECTX-SS-beta")],
        &digest,
    );

    let error = decide_bound(&evidence, &package, &Policy::v1()).expect_err("other taxa");

    assert_eq!(error.kind(), PackageErrorKind::Unresolved, "{error}");
    assert!(error.to_string().contains("other taxa"), "{error}");
}

#[test]
fn without_a_package_the_reference_is_reported_as_not_verified() {
    let evidence = saved(&packaged_evidence(&"c".repeat(64)));
    let decision: Value =
        serde_json::from_str(&decide(&evidence, &Policy::v1()).to_json()).unwrap();

    assert_eq!(decision["reference_package"]["status"], "not_verified");
}
