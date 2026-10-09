//! The decision core accepts evidence only when it is bound to the package it is given, and it
//! records that package's identity in the decision. Bound evidence must say what the package says
//! about every reference it names; a contradiction is refused, never rewritten.

mod common;

use common::package::{Synthetic, CONVERTED_FROM_SHA256, DISCLAIMER, PACKAGE_VERSION};
use common::packaged_evidence;
use serde_json::Value;
use terobacter::decision::{decide, decide_bound, Policy};
use terobacter::evidence::SavedEvidence;
use terobacter::reference::{PackageErrorKind, VerifiedPackage};

fn saved(json: &Value) -> SavedEvidence {
    SavedEvidence::from_json(&json.to_string()).expect("evidence loads")
}

fn verified() -> VerifiedPackage {
    Synthetic::new().verify().expect("package verifies")
}

fn decision(evidence: &Value, package: &VerifiedPackage) -> Value {
    let decision =
        decide_bound(&saved(evidence), package, &Policy::v1()).expect("evidence is bound");
    serde_json::from_str(&decision.to_json()).unwrap()
}

/// The package's own evidence with an edit, and the message binding it gives.
fn refused(edit: impl FnOnce(&mut Value)) -> String {
    let package = verified();
    let mut evidence = packaged_evidence(&package.package().manifest_sha256);
    edit(&mut evidence);
    let error = decide_bound(&saved(&evidence), &package, &Policy::v1()).expect_err("contradicts");
    assert_eq!(error.kind(), PackageErrorKind::Unresolved, "{error}");
    error.to_string()
}

/// The row for `reference_id`, which the fixture's primary component holds.
fn row<'a>(evidence: &'a mut Value, reference_id: &str) -> &'a mut Value {
    evidence["ranked_reference_evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["reference_id"] == reference_id)
        .unwrap_or_else(|| panic!("no row {reference_id}"))
}

/// Every row in which `field` is `from`, so a renamed taxon stays consistent within the evidence.
fn rows_with<'a>(
    evidence: &'a mut Value,
    field: &'a str,
    from: &'a str,
) -> impl Iterator<Item = &'a mut Value> + 'a {
    evidence["ranked_reference_evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .filter(move |row| row[field] == from)
}

/// A subspecies type anchor whose accession is also a genomovar definition member.
const SUBSPECIES_ANCHOR: &str = "primary_whole:GCA_900000002.1";
/// A diversity reference of a species without subspecies.
const SPECIES_ONLY: &str = "primary_whole:GCA_900000005.1";

#[test]
fn evidence_bound_to_the_verified_package_records_its_identity_and_components() {
    let package = verified();
    let digest = package.package().manifest_sha256.clone();

    let decision = decision(&packaged_evidence(&digest), &package);

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
fn evidence_bound_to_the_version_1_manifest_binds_through_the_attested_conversion() {
    let package = verified();
    assert_eq!(
        package.package().converted_from_sha256.as_deref(),
        Some(CONVERTED_FROM_SHA256)
    );

    let decision = decision(&packaged_evidence(CONVERTED_FROM_SHA256), &package);

    let report = &decision["reference_package"];
    assert_eq!(report["status"], "verified");
    assert_eq!(
        report["package"]["manifest_sha256"],
        package.package().manifest_sha256
    );
    assert_eq!(
        report["package"]["converted_from_sha256"],
        CONVERTED_FROM_SHA256
    );
    assert_eq!(
        decision["input"]["reference_package_sha256"],
        CONVERTED_FROM_SHA256
    );
}

#[test]
fn a_package_without_a_conversion_record_refuses_evidence_bound_to_the_version_1_manifest() {
    let mut synthetic = Synthetic::new();
    synthetic.without_conversion();
    let package = synthetic.verify().expect("a native export verifies");
    assert_eq!(package.package().converted_from_sha256, None);

    let evidence = saved(&packaged_evidence(CONVERTED_FROM_SHA256));
    let error = decide_bound(&evidence, &package, &Policy::v1()).expect_err("not bound");

    assert_eq!(error.kind(), PackageErrorKind::Unresolved, "{error}");
    assert!(
        error.to_string().contains("another reference package"),
        "{error}"
    );
}

#[test]
fn evidence_bound_to_another_package_is_unresolved() {
    let evidence = saved(&packaged_evidence(&"c".repeat(64)));

    let error = decide_bound(&evidence, &verified(), &Policy::v1()).expect_err("not bound");

    assert_eq!(error.kind(), PackageErrorKind::Unresolved, "{error}");
    assert!(
        error.to_string().contains("another reference package"),
        "{error}"
    );
}

#[test]
fn evidence_naming_a_reference_the_component_lacks_is_unresolved() {
    let message = refused(|evidence| {
        row(evidence, SPECIES_ONLY)["reference_id"] = "primary_whole:GCA_900000009.1".into();
    });
    assert!(
        message.contains("GCA_900000009.1 is not a reference of component primary_whole"),
        "{message}"
    );

    // GCA_900000006.1 is packaged, but only as a genomovar definition member.
    let message = refused(|evidence| {
        row(evidence, SPECIES_ONLY)["reference_id"] = "genomovar_chromosome:GCA_900000006.1".into();
    });
    assert!(
        message.contains("is not a reference of component primary_whole"),
        "{message}"
    );
}

#[test]
fn evidence_over_part_of_the_component_is_unresolved() {
    let message = refused(|evidence| {
        let rows = evidence["ranked_reference_evidence"]
            .as_array_mut()
            .unwrap();
        rows.pop();
        let count = rows.len();
        for field in ["reference_count", "raw_hit_count", "unique_hit_count"] {
            evidence[field] = count.into();
        }
    });
    assert!(
        message.contains("the evidence counts 4 references, but component primary_whole holds 5"),
        "{message}"
    );
}

#[test]
fn evidence_giving_another_species_is_unresolved() {
    let message = refused(|evidence| {
        let row = row(evidence, SPECIES_ONLY);
        row["species_id"] = "SYN-SP01".into();
        row["species_name"] = "Enterobacter syntheticus".into();
    });
    assert!(
        message.contains(
            "GCA_900000005.1: the evidence gives species SYN-SP01 (Enterobacter syntheticus), \
             the package gives SYN-SP02 (Enterobacter fictus)"
        ),
        "{message}"
    );
}

#[test]
fn evidence_giving_a_stale_species_name_is_unresolved() {
    let message = refused(|evidence| {
        for row in rows_with(evidence, "species_id", "SYN-SP02") {
            row["species_name"] = "Enterobacter fictitius".into();
        }
    });
    assert!(
        message.contains("the evidence gives species SYN-SP02 (Enterobacter fictitius)"),
        "{message}"
    );
}

#[test]
fn evidence_giving_another_subspecies_name_is_unresolved() {
    let message = refused(|evidence| {
        for row in rows_with(evidence, "subspecies_id", "SYN-SS02") {
            row["subspecies_name"] = "Enterobacter syntheticus subsp. altera".into();
        }
    });
    assert!(
        message.contains(
            "the evidence gives subspecies SYN-SS02 (Enterobacter syntheticus subsp. altera), \
             the package gives SYN-SS02 (Enterobacter syntheticus subsp. alter)"
        ),
        "{message}"
    );
}

#[test]
fn evidence_adding_or_dropping_a_subspecies_is_unresolved() {
    let message = refused(|evidence| {
        let row = row(evidence, SPECIES_ONLY);
        row["subspecies_id"] = "SYN-SS09".into();
        row["subspecies_name"] = "Enterobacter fictus subsp. novus".into();
    });
    assert!(
        message.contains(
            "the evidence gives subspecies SYN-SS09 (Enterobacter fictus subsp. novus), \
             the package gives none"
        ),
        "{message}"
    );

    let message = refused(|evidence| {
        let row = row(evidence, SUBSPECIES_ANCHOR);
        row["subspecies_id"] = "".into();
        row["subspecies_name"] = "".into();
        row["name_status"] = "validly_published_correct_name".into();
    });
    assert!(
        message.contains("the evidence gives subspecies none, the package gives SYN-SS02"),
        "{message}"
    );
}

#[test]
fn evidence_giving_another_name_status_is_unresolved() {
    let message = refused(|evidence| {
        row(evidence, SUBSPECIES_ANCHOR)["name_status"] = "validly_published_correct_name".into();
    });
    assert!(
        message.contains(
            "the evidence gives name status validly_published_correct_name, the package gives \
             approved_project_subspecies_proposal_not_validated_combination"
        ),
        "{message}"
    );
}

#[test]
fn evidence_giving_another_role_is_unresolved() {
    let message = refused(|evidence| {
        row(evidence, SUBSPECIES_ANCHOR)["reference_role"] = "diversity".into();
    });
    assert!(
        message.contains("the evidence gives role diversity, the package gives type_anchor"),
        "{message}"
    );

    let message = refused(|evidence| {
        row(evidence, SPECIES_ONLY)["reference_role"] = "type_anchor".into();
    });
    assert!(
        message.contains("the evidence gives role type_anchor, the package gives diversity"),
        "{message}"
    );
}

#[test]
fn evidence_giving_a_primary_reference_a_lineage_is_unresolved() {
    let message = refused(|evidence| {
        row(evidence, SPECIES_ONLY)["lineage_id"] = "SYN-LN01".into();
    });
    assert!(
        message.contains("the evidence gives lineage SYN-LN01"),
        "{message}"
    );
}

#[test]
fn without_a_package_the_reference_is_reported_as_not_verified() {
    let evidence = saved(&packaged_evidence(&"c".repeat(64)));
    let decision: Value =
        serde_json::from_str(&decide(&evidence, &Policy::v1()).to_json()).unwrap();

    assert_eq!(decision["reference_package"]["status"], "not_verified");
}
