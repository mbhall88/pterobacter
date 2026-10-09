//! Authenticating a reference package: each rejection is a named failure that a caller can tell
//! apart from the others. The packages are synthetic, built in memory, and damaged one thing at
//! a time.

mod common;

use common::package::{
    licence, MemorySource, Synthetic, DISCLAIMER, EXPORT_PATH, LICENCE_PATH, MANIFEST,
    PACKAGE_VERSION,
};
use serde_json::json;
use terobacter::reference::{verify_package, PackageErrorKind};

fn assert_rejected(synthetic: &Synthetic, kind: PackageErrorKind, names: &str) {
    let error = synthetic
        .verify()
        .expect_err("the package should be rejected");
    assert_eq!(error.kind(), kind, "{error}");
    assert!(error.to_string().contains(names), "{error}");
}

#[test]
fn a_complete_package_verifies_with_its_identity_components_and_licence() {
    let synthetic = Synthetic::new();
    let package = synthetic.verify().expect("a complete package verifies");

    assert_eq!(
        package.package().manifest_sha256,
        synthetic.manifest_sha256()
    );
    assert_eq!(package.package().version, PACKAGE_VERSION);
    assert_eq!(
        package.licence().ncbi_submitter_rights_disclaimer,
        DISCLAIMER
    );
    for id in ["primary_whole", "genomovar_chromosome"] {
        let component = package.component(id).expect("both components are present");
        assert_eq!(component.databases.len(), 3, "{id}");
        assert_eq!(component.builds.len(), 1, "{id}");
    }
}

#[test]
fn an_altered_database_is_rejected_as_altered() {
    let mut synthetic = Synthetic::new();
    synthetic.alter("databases/primary_whole/index.db");
    assert_rejected(
        &synthetic,
        PackageErrorKind::Altered,
        "databases/primary_whole/index.db",
    );
}

#[test]
fn altered_metadata_and_licence_are_rejected_as_altered() {
    for path in [EXPORT_PATH, LICENCE_PATH] {
        let mut synthetic = Synthetic::new();
        synthetic.alter(path);
        assert_rejected(&synthetic, PackageErrorKind::Altered, path);
    }
}

#[test]
fn a_truncated_database_is_rejected_as_truncated() {
    let mut synthetic = Synthetic::new();
    synthetic.truncate("databases/genomovar_chromosome/sketches.db");
    assert_rejected(
        &synthetic,
        PackageErrorKind::Truncated,
        "databases/genomovar_chromosome/sketches.db",
    );
}

#[test]
fn a_listed_file_that_is_absent_is_rejected_as_missing() {
    let mut synthetic = Synthetic::new();
    synthetic.remove("databases/primary_whole/markers.bin");
    assert_rejected(
        &synthetic,
        PackageErrorKind::Missing,
        "databases/primary_whole/markers.bin",
    );
}

#[test]
fn a_package_without_a_manifest_is_rejected_as_missing() {
    let mut files = Synthetic::new().files();
    files.remove(MANIFEST);
    let expected = Synthetic::new().manifest_sha256();
    let error = verify_package(&MemorySource(files), &expected).expect_err("no manifest");

    assert_eq!(error.kind(), PackageErrorKind::Missing);
    assert!(error.to_string().contains(MANIFEST), "{error}");
}

#[test]
fn a_manifest_that_is_consistent_but_not_the_expected_one_is_unexpected() {
    let expected = Synthetic::new().manifest_sha256();
    let mut replaced = Synthetic::new();
    replaced.set_manifest("package_id", json!("terobacter-reference-replacement"));
    let error =
        verify_package(&replaced.source(), &expected).expect_err("a replaced manifest is refused");

    assert_eq!(error.kind(), PackageErrorKind::Unexpected, "{error}");
    assert!(error.to_string().contains(&expected), "{error}");
}

#[test]
fn the_expected_digest_is_checked_before_the_manifest_is_parsed() {
    let expected = Synthetic::new().manifest_sha256();
    let mut damaged = Synthetic::new();
    damaged.replace_manifest_bytes(b"{ not json");
    let error = verify_package(&damaged.source(), &expected).expect_err("a damaged manifest");

    assert_eq!(error.kind(), PackageErrorKind::Unexpected, "{error}");
}

#[test]
fn another_package_schema_is_rejected_as_incompatible() {
    let mut synthetic = Synthetic::new();
    synthetic.set_manifest(
        "schema_version",
        json!("0.9.0-dev-portable-reference-package"),
    );
    assert_rejected(&synthetic, PackageErrorKind::Incompatible, "schema");
}

#[test]
fn a_manifest_that_is_not_json_is_rejected_as_incomplete() {
    let mut synthetic = Synthetic::new();
    synthetic.replace_manifest_bytes(b"{ not json");
    assert_rejected(&synthetic, PackageErrorKind::Incomplete, MANIFEST);
}

#[test]
fn a_package_without_a_licence_manifest_is_incomplete() {
    let mut synthetic = Synthetic::new();
    synthetic.drop_listing(LICENCE_PATH);
    assert_rejected(&synthetic, PackageErrorKind::Incomplete, "licence");
}

#[test]
fn a_licence_without_the_ncbi_disclaimer_is_incomplete() {
    let mut synthetic = Synthetic::new();
    synthetic.replace_licence(licence(PACKAGE_VERSION, "  "));
    assert_rejected(&synthetic, PackageErrorKind::Incomplete, "disclaimer");
}

#[test]
fn a_licence_of_another_schema_is_incompatible_even_if_its_fields_differ() {
    let mut synthetic = Synthetic::new();
    synthetic.replace_licence(json!({
        "schema_version": "0.9.0-reference-licence-manifest",
        "package_version": PACKAGE_VERSION,
        "rights_statement": DISCLAIMER,
    }));
    assert_rejected(&synthetic, PackageErrorKind::Incompatible, "schema");
}

#[test]
fn a_licence_for_another_package_version_is_incomplete() {
    let mut synthetic = Synthetic::new();
    synthetic.replace_licence(licence("2025-01-01.other", DISCLAIMER));
    assert_rejected(&synthetic, PackageErrorKind::Incomplete, "2025-01-01.other");
}

#[test]
fn a_component_without_a_listed_database_is_incomplete() {
    let mut synthetic = Synthetic::new();
    synthetic.drop_listing("databases/genomovar_chromosome/markers.bin");
    assert_rejected(
        &synthetic,
        PackageErrorKind::Incomplete,
        "genomovar_chromosome",
    );
}

#[test]
fn unsafe_or_duplicate_listed_paths_are_incomplete() {
    let mut escaping = Synthetic::new();
    escaping.rename_listing("tables/named_registry.tsv", "../named_registry.tsv");
    assert_rejected(
        &escaping,
        PackageErrorKind::Incomplete,
        "../named_registry.tsv",
    );

    let mut duplicate = Synthetic::new();
    duplicate.rename_listing(
        "tables/named_registry.tsv",
        "build/primary_whole/build.json",
    );
    assert_rejected(
        &duplicate,
        PackageErrorKind::Incomplete,
        "build/primary_whole/build.json",
    );
}
