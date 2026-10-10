//! The frozen genomovar definition `2026-10-02-v1` as an authenticated package exposes it: 25
//! groups of 173 historical chromosome members under `ECTX-SS007`, pinned by a canonical
//! membership digest and the frozen source hashes of its members. A package with any other
//! definition supports no genomovar call.
mod common;

use std::collections::BTreeMap;

use common::package::Synthetic;
use terobacter::genomovar::{
    member_source_sha256, Definition, DEFINITION_VERSION, FROZEN_MEMBERSHIP_SHA256,
    FROZEN_MEMBER_SOURCE_SHA256,
};

fn groups(spec: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
    spec.iter()
        .map(|(g, ms)| (g.to_string(), ms.iter().map(|m| m.to_string()).collect()))
        .collect()
}

#[test]
fn a_package_without_the_frozen_definition_supports_no_genomovar() {
    let package = Synthetic::new().verify().unwrap();
    let error = Definition::from_package(&package).unwrap_err().to_string();
    assert!(error.contains("frozen genomovar definition"), "{error}");
    let component = package.component("genomovar_chromosome").unwrap();
    assert_ne!(member_source_sha256(component), FROZEN_MEMBER_SOURCE_SHA256);
}

#[test]
fn the_membership_digest_ignores_order_but_not_membership() {
    let a = Definition::new(
        DEFINITION_VERSION,
        groups(&[("GV01", &["x", "y"]), ("GV02", &["z"])]),
    )
    .unwrap();
    let reordered = Definition::new(
        DEFINITION_VERSION,
        groups(&[("GV02", &["z"]), ("GV01", &["y", "x"])]),
    )
    .unwrap();
    let moved = Definition::new(
        DEFINITION_VERSION,
        groups(&[("GV01", &["x"]), ("GV02", &["y", "z"])]),
    )
    .unwrap();
    assert_eq!(a.membership_sha256(), reordered.membership_sha256());
    assert_ne!(a.membership_sha256(), moved.membership_sha256());
    assert_ne!(a.membership_sha256(), FROZEN_MEMBERSHIP_SHA256);
}

/// Run against a private copy of the frozen package:
/// `TEROBACTER_FROZEN_PACKAGE=<dir> TEROBACTER_FROZEN_PACKAGE_SHA256=<sha> cargo test --test
/// genomovar_definition -- --ignored`. The package is not distributed with the repository.
#[test]
#[ignore = "needs a private copy of the frozen reference package"]
fn the_frozen_package_carries_the_pinned_definition() {
    let directory = std::env::var("TEROBACTER_FROZEN_PACKAGE").unwrap();
    let sha256 = std::env::var("TEROBACTER_FROZEN_PACKAGE_SHA256").unwrap();
    let workflow = terobacter::cli::assembly::AssemblyWorkflow::open(
        std::path::Path::new(&directory),
        &sha256,
    )
    .unwrap();
    // Opening the definition checks the membership and member source digests.
    let definition = workflow.genomovar_definition().unwrap();
    assert_eq!(definition.version(), DEFINITION_VERSION);
    assert_eq!(definition.groups().len(), 25);
    assert_eq!(definition.member_count(), 173);
    assert_eq!(
        definition
            .groups()
            .values()
            .filter(|members| members.len() == 1)
            .count(),
        15
    );
    assert_eq!(definition.membership_sha256(), FROZEN_MEMBERSHIP_SHA256);
    assert_eq!(
        workflow.genomovar_search_parameters().unwrap(),
        terobacter::genomovar::SEARCH_PARAMETERS
    );
}
