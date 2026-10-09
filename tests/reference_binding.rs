//! The decision core accepts evidence only when it is bound to the package it is given, and it
//! records that package's identity in the decision.

mod common;

use common::package::{Synthetic, DISCLAIMER, PACKAGE_VERSION};
use common::{evidence_json, row};
use serde_json::Value;
use terobacter::decision::{decide, decide_bound, Policy};
use terobacter::evidence::SavedEvidence;
use terobacter::reference::{PackageErrorKind, VerifiedPackage};

fn evidence_bound_to(package_sha256: &str) -> SavedEvidence {
    let mut json = evidence_json(&[row("GCF_000000001", Some("cloacae"), 98.5, 95.0, 96.0)]);
    json["provenance"]["reference_package_sha256"] = package_sha256.into();
    SavedEvidence::from_json(&json.to_string()).expect("evidence loads")
}

fn bound_decision_json(evidence: &SavedEvidence, package: &VerifiedPackage) -> Value {
    let decision = decide_bound(evidence, package, &Policy::v1()).expect("evidence is bound");
    serde_json::from_str(&decision.to_json()).expect("decision is JSON")
}

#[test]
fn evidence_bound_to_the_verified_package_records_its_identity_and_components() {
    let package = Synthetic::new().verify().expect("package verifies");
    let digest = &package.package().manifest_sha256;
    let evidence = evidence_bound_to(digest);
    let decision = bound_decision_json(&evidence, &package);

    let report = &decision["reference_package"];
    assert_eq!(report["status"], "verified");
    assert_eq!(report["package"]["version"], PACKAGE_VERSION);
    assert_eq!(report["package"]["manifest_sha256"], *digest);
    assert_eq!(
        report["licence"]["ncbi_submitter_rights_disclaimer"],
        DISCLAIMER
    );
    assert_eq!(decision["input"]["reference_package_sha256"], *digest);
    for id in ["primary_whole", "genomovar_chromosome"] {
        let index = &report["components"][id]["databases"]["index.db"];
        assert_eq!(index["sha256"].as_str().map(str::len), Some(64), "{id}");
    }
}

#[test]
fn evidence_bound_to_another_package_is_not_bound() {
    let package = Synthetic::new().verify().expect("package verifies");
    let evidence = evidence_bound_to(&"c".repeat(64));

    let error = decide_bound(&evidence, &package, &Policy::v1()).expect_err("not bound");
    assert_eq!(error.kind(), PackageErrorKind::NotBound, "{error}");
}

#[test]
fn without_a_package_the_reference_is_reported_as_not_verified() {
    let evidence = evidence_bound_to(&"c".repeat(64));
    let decision: Value =
        serde_json::from_str(&decide(&evidence, &Policy::v1()).to_json()).unwrap();

    assert_eq!(decision["reference_package"]["status"], "not_verified");
}
