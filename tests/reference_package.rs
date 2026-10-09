//! Authenticating a reference package: each rejection is a named failure that a caller can tell
//! apart from the others. The packages are synthetic, built in memory, and damaged one thing at
//! a time.

mod common;

use common::package::{
    sha256_hex, MemorySource, Synthetic, DISCLAIMER, EXPORT_PATH, LICENCE_PATH, LINEAGE_REGISTRY,
    MANIFEST, NAMED_REGISTRY, PACKAGE_VERSION, PINNED_MANIFEST_SHA256, PRIMARY_INDEX, SOURCE_PATH,
};
use serde_json::{json, Value};
use terobacter::reference::{verify_package, PackageErrorKind, RegistryPins};

const V1_SCHEMA: &str = "1.0.0-dev-portable-reference-package";
const PRIMARY_TABLE: &str = "metadata/tables/primary_references.tsv";
const GENOMOVAR_MARKERS: &str = "databases/genomovar_chromosome/markers.bin";
const GENOMOVAR_SKETCHES: &str = "databases/genomovar_chromosome/sketches.db";

#[test]
fn the_committed_manifest_has_its_pinned_digest() {
    let synthetic = Synthetic::new();
    assert_eq!(synthetic.manifest_sha256(), PINNED_MANIFEST_SHA256);
    synthetic
        .verify_with(PINNED_MANIFEST_SHA256, &RegistryPins::default())
        .expect("the committed package verifies against its pinned digest");
}

fn other_digest() -> String {
    "c".repeat(64)
}

fn assert_rejected(synthetic: &Synthetic, kind: PackageErrorKind, names: &str) {
    let error = synthetic
        .verify()
        .expect_err("the package should be rejected");
    assert_eq!(error.kind(), kind, "{error}");
    assert!(error.to_string().contains(names), "{error}");
}

/// Edit the source manifest and re-seal the two records that name its digest, as an export would.
fn edit_source(synthetic: &mut Synthetic, edit: impl FnOnce(&mut Value)) {
    synthetic.edit_json(SOURCE_PATH, edit);
    let digest = sha256_hex(synthetic.file(SOURCE_PATH));
    synthetic.edit_json(EXPORT_PATH, |export| {
        export["source_manifest_sha256"] = json!(digest)
    });
    synthetic.edit_json(LICENCE_PATH, |licence| {
        licence["source_manifest_sha256"] = json!(digest)
    });
}

/// Edit a table and re-seal every record that names it, so the table's own check is what rejects it.
fn edit_table(synthetic: &mut Synthetic, path: &str, edit: impl FnOnce(&str) -> String) {
    let text = String::from_utf8(synthetic.file(path).to_vec()).unwrap();
    let edited = edit(&text);
    assert_ne!(edited, text, "the edit changed nothing in {path}");
    replace_file(synthetic, path, edited.into_bytes());
}

/// Replace a listed file and re-seal every record that names it, so the file's own check is what rejects it.
fn replace_file(synthetic: &mut Synthetic, path: &str, bytes: Vec<u8>) {
    synthetic.rewrite(path, bytes);
    let source_path = source_path_of(synthetic, path);
    let len = synthetic.file(path).len() as u64;
    let digest = sha256_hex(synthetic.file(path));
    edit_source(synthetic, |source| {
        restate(source, &source_path, len, &digest)
    });
    sync_database_bytes(synthetic);
}

/// Keep the export's count of database bytes equal to the total the manifest lists.
fn sync_database_bytes(synthetic: &mut Synthetic) {
    let manifest: Value = serde_json::from_slice(&synthetic.files()[MANIFEST]).unwrap();
    let total: u64 = manifest["assets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|asset| {
            asset["roles"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|role| role.as_str().unwrap_or_default().starts_with("database:"))
        })
        .map(|asset| asset["bytes"].as_u64().unwrap())
        .sum();
    synthetic.edit_json(EXPORT_PATH, |export| {
        export["local_validation"]["database_bytes"] = json!(total)
    });
}

type IndexEntry = (String, u64, u64);

fn decode_index(bytes: &[u8]) -> Vec<IndexEntry> {
    let word = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
    let mut at = 8;
    (0..word(0))
        .map(|_| {
            let len = word(at) as usize;
            let label = String::from_utf8(bytes[at + 8..at + 8 + len].to_vec()).unwrap();
            let offset = word(at + 8 + len);
            let size = word(at + 16 + len);
            at += 24 + len;
            (label, offset, size)
        })
        .collect()
}

fn encode_index(entries: &[IndexEntry]) -> Vec<u8> {
    let mut bytes = (entries.len() as u64).to_le_bytes().to_vec();
    for (label, offset, size) in entries {
        bytes.extend((label.len() as u64).to_le_bytes());
        bytes.extend(label.as_bytes());
        bytes.extend(offset.to_le_bytes());
        bytes.extend(size.to_le_bytes());
    }
    bytes
}

/// Edit the primary skani index as its decoded entries, then re-seal it as an export would.
fn edit_index(synthetic: &mut Synthetic, edit: impl FnOnce(&mut Vec<IndexEntry>)) {
    let mut entries = decode_index(synthetic.file(PRIMARY_INDEX));
    edit(&mut entries);
    replace_file(synthetic, PRIMARY_INDEX, encode_index(&entries));
}

fn source_path_of(synthetic: &Synthetic, path: &str) -> String {
    let manifest: Value = serde_json::from_slice(&synthetic.files()[MANIFEST]).unwrap();
    manifest["assets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|asset| asset["path"] == path)
        .and_then(|asset| asset["source_path"].as_str())
        .expect("the table is listed with its source path")
        .to_owned()
}

/// Give every record under `value` that names `source_path` the new length and digest.
fn restate(value: &mut Value, source_path: &str, bytes: u64, digest: &str) {
    match value {
        Value::Object(record) => {
            if record.get("path") == Some(&json!(source_path)) {
                record.insert("bytes".into(), json!(bytes));
                record.insert("sha256".into(), json!(digest));
            }
            for child in record.values_mut() {
                restate(child, source_path, bytes, digest);
            }
        }
        Value::Array(items) => {
            for item in items {
                restate(item, source_path, bytes, digest);
            }
        }
        _ => {}
    }
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
        package.package().release_state,
        "development_proposal_not_distributable"
    );
    assert_eq!(
        package.licence().ncbi_submitter_rights_disclaimer,
        DISCLAIMER
    );
    assert_eq!(package.components().len(), 2);
    let primary = package
        .component("primary_whole")
        .expect("the primary component is present");
    let genomovar = package
        .component("genomovar_chromosome")
        .expect("the genomovar component is present");
    assert_eq!(primary.reference_role, "primary_named_reference_evidence");
    assert_eq!(primary.sequence_scope, "whole_assembly_as_deposited");
    assert_eq!(primary.databases.len(), 3);
    assert_eq!(primary.builds.len(), 1);
    assert_eq!(primary.references.len(), 2);
    assert_eq!(genomovar.reference_role, "genomovar_definition_member");
    assert_eq!(genomovar.sequence_scope, "historical_chromosome_record_set");
    assert_eq!(genomovar.databases.len(), 3);
    assert_eq!(genomovar.references.len(), 2);
}

#[test]
fn an_altered_database_is_rejected_as_altered() {
    let mut synthetic = Synthetic::new();
    synthetic.alter(PRIMARY_INDEX);
    assert_rejected(&synthetic, PackageErrorKind::Altered, PRIMARY_INDEX);
}

#[test]
fn altered_metadata_and_licence_are_rejected_as_altered() {
    for path in [SOURCE_PATH, EXPORT_PATH, LICENCE_PATH] {
        let mut synthetic = Synthetic::new();
        synthetic.alter(path);
        assert_rejected(&synthetic, PackageErrorKind::Altered, path);
    }
}

#[test]
fn a_truncated_database_is_rejected_as_truncated() {
    let mut synthetic = Synthetic::new();
    synthetic.truncate(GENOMOVAR_SKETCHES);
    assert_rejected(&synthetic, PackageErrorKind::Truncated, GENOMOVAR_SKETCHES);
}

#[test]
fn a_longer_database_than_listed_is_altered_not_truncated() {
    let mut synthetic = Synthetic::new();
    synthetic.extend(GENOMOVAR_SKETCHES);
    assert_rejected(&synthetic, PackageErrorKind::Altered, GENOMOVAR_SKETCHES);
}

#[test]
fn a_listed_file_that_is_absent_is_rejected_as_missing() {
    let mut synthetic = Synthetic::new();
    synthetic.remove(GENOMOVAR_MARKERS);
    assert_rejected(&synthetic, PackageErrorKind::Missing, GENOMOVAR_MARKERS);
}

#[test]
fn a_package_without_its_licence_is_rejected_as_missing() {
    let mut synthetic = Synthetic::new();
    synthetic.remove(LICENCE_PATH);
    assert_rejected(&synthetic, PackageErrorKind::Missing, LICENCE_PATH);
}

#[test]
fn a_package_without_a_manifest_is_rejected_as_missing() {
    let mut files = Synthetic::new().files();
    files.remove(MANIFEST);
    let expected = Synthetic::new().manifest_sha256();
    let error = verify_package(
        &MemorySource::new(files),
        &expected,
        &RegistryPins::default(),
    )
    .expect_err("no manifest");

    assert_eq!(error.kind(), PackageErrorKind::Missing);
    assert!(error.to_string().contains(MANIFEST), "{error}");
}

#[test]
fn a_manifest_that_is_consistent_but_not_the_expected_one_is_unexpected() {
    let expected = Synthetic::new().manifest_sha256();
    let mut replaced = Synthetic::new();
    replaced.edit_manifest(|manifest| {
        manifest["package_id"] = json!("terobacter-reference-replacement")
    });
    let error = replaced
        .verify_with(&expected, &RegistryPins::default())
        .expect_err("a replaced manifest is refused");

    assert_eq!(error.kind(), PackageErrorKind::Unexpected, "{error}");
    assert!(error.to_string().contains(&expected), "{error}");
}

#[test]
fn the_expected_digest_is_checked_before_the_manifest_is_parsed() {
    let expected = Synthetic::new().manifest_sha256();
    let mut damaged = Synthetic::new();
    damaged.replace_manifest_bytes(b"{ not json");
    let error = damaged
        .verify_with(&expected, &RegistryPins::default())
        .expect_err("a damaged manifest");

    assert_eq!(error.kind(), PackageErrorKind::Unexpected, "{error}");
}

#[test]
fn another_package_schema_is_rejected_as_incompatible() {
    let mut synthetic = Synthetic::new();
    synthetic.edit_manifest(|manifest| {
        manifest["schema_version"] = json!("0.9.0-dev-portable-reference-package")
    });
    assert_rejected(&synthetic, PackageErrorKind::Incompatible, "schema");
}

#[test]
fn the_frozen_v1_package_schema_is_incompatible() {
    let mut synthetic = Synthetic::new();
    synthetic.edit_manifest(|manifest| manifest["schema_version"] = json!(V1_SCHEMA));
    assert_rejected(&synthetic, PackageErrorKind::Incompatible, V1_SCHEMA);
}

#[test]
fn a_manifest_that_is_not_json_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    synthetic.replace_manifest_bytes(b"{ not json");
    assert_rejected(&synthetic, PackageErrorKind::Inconsistent, MANIFEST);
}

#[test]
fn a_manifest_without_a_schema_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    synthetic.edit_manifest(|manifest| {
        manifest.as_object_mut().unwrap().remove("schema_version");
    });
    assert_rejected(&synthetic, PackageErrorKind::Inconsistent, "schema_version");
}

#[test]
fn a_licence_without_the_ncbi_disclaimer_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    synthetic.edit_json(LICENCE_PATH, |licence| {
        licence["ncbi_submitter_rights_disclaimer"] = json!("  ")
    });
    assert_rejected(&synthetic, PackageErrorKind::Inconsistent, "disclaimer");
}

#[test]
fn a_licence_of_another_schema_is_incompatible() {
    let mut synthetic = Synthetic::new();
    synthetic.edit_json(LICENCE_PATH, |licence| {
        licence["schema_version"] = json!("0.9.0-reference-licence-manifest")
    });
    assert_rejected(&synthetic, PackageErrorKind::Incompatible, "schema");
}

#[test]
fn a_licence_for_another_package_version_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    synthetic.edit_json(LICENCE_PATH, |licence| {
        licence["package_version"] = json!("2025-01-01.other")
    });
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "2025-01-01.other",
    );
}

#[test]
fn a_licence_naming_another_source_manifest_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    let other = other_digest();
    synthetic.edit_json(LICENCE_PATH, |licence| {
        licence["source_manifest_sha256"] = json!(other)
    });
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "another source manifest",
    );
}

#[test]
fn a_source_manifest_that_is_empty_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    synthetic.edit_json(SOURCE_PATH, |source| *source = json!({}));
    assert_rejected(&synthetic, PackageErrorKind::Inconsistent, "schema_version");
}

#[test]
fn an_export_attestation_that_is_empty_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    synthetic.edit_json(EXPORT_PATH, |export| *export = json!({}));
    assert_rejected(&synthetic, PackageErrorKind::Inconsistent, "malformed");
}

#[test]
fn an_export_attestation_that_is_not_a_valid_validation_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    synthetic.edit_json(EXPORT_PATH, |export| {
        export["local_validation"]["valid"] = json!(false)
    });
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "valid local validation",
    );
}

#[test]
fn an_export_attestation_with_another_scope_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    synthetic.edit_json(EXPORT_PATH, |export| {
        export["scope"] = json!("full_external_validation")
    });
    assert_rejected(&synthetic, PackageErrorKind::Inconsistent, "another scope");
}

#[test]
fn an_export_attestation_for_another_source_manifest_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    let other = other_digest();
    synthetic.edit_json(EXPORT_PATH, |export| {
        export["source_manifest_sha256"] = json!(other)
    });
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "another source manifest",
    );
}

#[test]
fn an_export_attestation_that_counts_other_database_bytes_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    synthetic.edit_json(EXPORT_PATH, |export| {
        export["local_validation"]["database_bytes"] = json!(594)
    });
    assert_rejected(&synthetic, PackageErrorKind::Inconsistent, "database bytes");
}

#[test]
fn a_manifest_naming_another_package_version_than_its_source_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    synthetic.edit_manifest(|manifest| manifest["package_version"] = json!("2025-01-01.other"));
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "another package or version",
    );
}

#[test]
fn a_source_count_the_tables_do_not_bear_out_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    edit_source(&mut synthetic, |source| {
        source["counts"]["primary_whole_references"] = json!(3)
    });
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "counts differ from the source manifest",
    );
}

#[test]
fn a_reference_with_another_role_or_scope_is_inconsistent() {
    for (field, value) in [
        ("reference_role", "type_anchor"),
        ("sequence_scope", "whole_assembly_as_supplied"),
    ] {
        let mut synthetic = Synthetic::new();
        edit_source(&mut synthetic, |source| {
            source["references"][0][field] = json!(value)
        });
        assert_rejected(
            &synthetic,
            PackageErrorKind::Inconsistent,
            "does not match component",
        );
    }
}

#[test]
fn a_named_registry_with_a_stale_name_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    edit_table(&mut synthetic, NAMED_REGISTRY, |text| {
        text.replace("subsp. beta", "subsp. delta")
    });
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "differs from its source annotation",
    );
}

#[test]
fn a_named_registry_with_a_wrong_parent_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    edit_table(&mut synthetic, NAMED_REGISTRY, |text| {
        text.replace("subspecies\tECTX-SP-alpha", "subspecies\tECTX-LN-gamma")
    });
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "differs from its source annotation",
    );
}

#[test]
fn a_table_with_another_header_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    edit_table(&mut synthetic, NAMED_REGISTRY, |text| {
        text.replace("primary140_type_count", "primary140_types")
    });
    assert_rejected(&synthetic, PackageErrorKind::Inconsistent, "header");
}

#[test]
fn a_table_that_repeats_a_row_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    edit_table(&mut synthetic, NAMED_REGISTRY, |text| {
        let row = text.lines().nth(1).unwrap();
        format!("{}\n{row}\n", text.trim_end())
    });
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "do not give the counts",
    );
}

#[test]
fn a_primary_table_naming_another_subspecies_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    edit_table(&mut synthetic, PRIMARY_TABLE, |text| {
        text.replace("\tbeta\t", "\tgamma\t")
    });
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "differs from the source manifest",
    );
}

#[test]
fn a_component_without_a_listed_database_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    let removed = synthetic.file(GENOMOVAR_MARKERS).len() as u64;
    synthetic.drop_listing(GENOMOVAR_MARKERS);
    synthetic.edit_json(EXPORT_PATH, |export| {
        let total = export["local_validation"]["database_bytes"]
            .as_u64()
            .unwrap();
        export["local_validation"]["database_bytes"] = json!(total - removed);
    });
    assert_rejected(&synthetic, PackageErrorKind::Inconsistent, "recipe");
}

#[test]
fn a_skani_index_that_repeats_a_label_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    edit_index(&mut synthetic, |entries| {
        entries[1].0 = entries[0].0.clone()
    });
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "the skani index repeats",
    );
}

#[test]
fn a_skani_index_with_a_gap_or_overlap_in_offsets_is_inconsistent() {
    let mut gap = Synthetic::new();
    edit_index(&mut gap, |entries| entries[1].1 += 1);
    assert_rejected(
        &gap,
        PackageErrorKind::Inconsistent,
        "the skani index places",
    );

    let mut overlap = Synthetic::new();
    edit_index(&mut overlap, |entries| entries[1].1 -= 1);
    assert_rejected(
        &overlap,
        PackageErrorKind::Inconsistent,
        "the skani index places",
    );
}

#[test]
fn a_skani_index_whose_sizes_do_not_cover_the_sketches_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    edit_index(&mut synthetic, |entries| entries.last_mut().unwrap().2 -= 1);
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "do not cover sketches.db",
    );
}

#[test]
fn a_skani_index_that_names_an_unknown_reference_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    edit_index(&mut synthetic, |entries| {
        entries[0].0 = "not-a-reference".into()
    });
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "the skani index names",
    );
}

#[test]
fn a_skani_index_that_omits_a_reference_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    edit_index(&mut synthetic, |entries| {
        entries[0].2 += entries[1].2;
        entries.remove(1);
    });
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "the skani index does not name",
    );
}

#[test]
fn a_skani_index_with_trailing_bytes_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    let mut bytes = synthetic.file(PRIMARY_INDEX).to_vec();
    bytes.push(0);
    replace_file(&mut synthetic, PRIMARY_INDEX, bytes);
    assert_rejected(&synthetic, PackageErrorKind::Inconsistent, "trailing bytes");
}

#[test]
fn a_skani_index_with_an_impossible_count_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    let mut bytes = synthetic.file(PRIMARY_INDEX).to_vec();
    bytes[..8].copy_from_slice(&u64::MAX.to_le_bytes());
    replace_file(&mut synthetic, PRIMARY_INDEX, bytes);
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "impossible sequence count",
    );
}

#[test]
fn a_skani_index_label_longer_than_the_bytes_that_follow_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    let mut bytes = synthetic.file(PRIMARY_INDEX).to_vec();
    bytes[8..16].copy_from_slice(&(1u64 << 40).to_le_bytes());
    replace_file(&mut synthetic, PRIMARY_INDEX, bytes);
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "ends before its entries do",
    );
}

#[test]
fn an_unlisted_file_is_rejected_as_unlisted() {
    let mut synthetic = Synthetic::new();
    synthetic.add_unlisted("databases/primary_whole/extra.tsv", b"extra");
    assert_rejected(
        &synthetic,
        PackageErrorKind::Unlisted,
        "databases/primary_whole/extra.tsv",
    );
}

#[test]
fn unsafe_or_duplicate_listed_paths_are_inconsistent() {
    for unsafe_path in [
        "../named_registry.tsv",
        "/named_registry.tsv",
        "metadata//named_registry.tsv",
    ] {
        let mut synthetic = Synthetic::new();
        synthetic.rename_listing(NAMED_REGISTRY, unsafe_path);
        assert_rejected(&synthetic, PackageErrorKind::Inconsistent, unsafe_path);
    }

    let mut duplicate = Synthetic::new();
    duplicate.rename_listing(NAMED_REGISTRY, LINEAGE_REGISTRY);
    assert_rejected(&duplicate, PackageErrorKind::Inconsistent, LINEAGE_REGISTRY);
}

#[test]
fn unsafe_listed_paths_are_refused_before_any_listed_file_is_opened() {
    let mut synthetic = Synthetic::new();
    synthetic.rename_listing(NAMED_REGISTRY, "../named_registry.tsv");
    let expected = synthetic.manifest_sha256();
    let source = synthetic.source();

    let error = verify_package(&source, &expected, &RegistryPins::default())
        .expect_err("an unsafe path is refused");

    assert_eq!(error.kind(), PackageErrorKind::Inconsistent, "{error}");
    assert_eq!(source.opened(), [MANIFEST]);
}

#[test]
fn a_pinned_registry_table_with_another_digest_is_unexpected() {
    let synthetic = Synthetic::new();
    let pins = RegistryPins {
        named_registry_sha256: Some(other_digest()),
        ..RegistryPins::default()
    };

    let error = synthetic
        .verify_with(&synthetic.manifest_sha256(), &pins)
        .expect_err("a pin that differs is refused");

    assert_eq!(error.kind(), PackageErrorKind::Unexpected, "{error}");
    assert!(error.to_string().contains(NAMED_REGISTRY), "{error}");
}

#[test]
fn registry_pins_that_match_the_listed_tables_verify() {
    let synthetic = Synthetic::new();
    let pins = RegistryPins {
        named_registry_sha256: Some(sha256_hex(synthetic.file(NAMED_REGISTRY))),
        lineage_registry_sha256: Some(sha256_hex(synthetic.file(LINEAGE_REGISTRY))),
    };

    synthetic
        .verify_with(&synthetic.manifest_sha256(), &pins)
        .expect("the pinned tables are the listed ones");
}
