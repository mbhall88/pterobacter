//! Authenticating a reference package: each rejection is a named failure that a caller can tell
//! apart from the others. The packages are synthetic, built in memory, and damaged one thing at
//! a time.

mod common;

use common::package::{
    sha256_hex, MemorySource, Synthetic, CONVERTED_FROM_PATH, CONVERTED_FROM_SHA256, DISCLAIMER,
    EXPORT_PATH, LICENCE_PATH, LINEAGE_REGISTRY, MANIFEST, NAMED_REGISTRY, PACKAGE_VERSION,
    PINNED_MANIFEST_SHA256, PRIMARY_INDEX, SOURCE_PATH,
};
use serde_json::{json, Value};
use terobacter::reference::{verify_package, PackageErrorKind, Rank, RegistryPins};

const V1_SCHEMA: &str = "1.0.0-dev-portable-reference-package";
const PRIMARY_TABLE: &str = "metadata/tables/primary_references.tsv";
const GENOMOVAR_MARKERS: &str = "databases/genomovar_chromosome/markers.bin";
const GENOMOVAR_SKETCHES: &str = "databases/genomovar_chromosome/sketches.db";
const COHORT_TABLE: &str = "metadata/tables/cohort_coverage.tsv";
const HISTORICAL_TABLE: &str = "metadata/tables/historical_types.tsv";
const ASSIGNMENTS_TABLE: &str = "metadata/tables/assignments.tsv";
const CROSSWALK_TABLE: &str = "metadata/tables/genomovar_crosswalk.tsv";
/// The message every disagreement between the source manifest and its tables gives.
const UNDERIVED: &str = "differs from the record the packaged tables derive";

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
    assert_eq!(primary.references.len(), 5);
    assert_eq!(genomovar.reference_role, "genomovar_definition_member");
    assert_eq!(genomovar.sequence_scope, "historical_chromosome_record_set");
    assert_eq!(genomovar.databases.len(), 3);
    assert_eq!(genomovar.references.len(), 4);
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
        assert_rejected(&synthetic, PackageErrorKind::Inconsistent, UNDERIVED);
    }
}

#[test]
fn a_named_registry_with_a_stale_name_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    edit_table(&mut synthetic, NAMED_REGISTRY, |text| {
        text.replace("subsp. alter\t", "subsp. altera\t")
    });
    // The primary table names the stratum by its registry name, so the references lose it.
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "primary_whole:GCA_900000002.1 names no species or subspecies concept",
    );
}

#[test]
fn a_named_registry_with_a_wrong_parent_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    edit_table(&mut synthetic, NAMED_REGISTRY, |text| {
        text.replace(
            "SYN-SS02\tsubspecies\tSYN-SP01",
            "SYN-SS02\tsubspecies\tSYN-SP02",
        )
    });
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "primary_whole:GCA_900000002.1 has another species than its assignment row",
    );
}

#[test]
fn a_table_with_another_header_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    edit_table(&mut synthetic, NAMED_REGISTRY, |text| {
        text.replace("primary140_type_count", "primary140_types")
    });
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "named_registry has no primary140_type_count column",
    );
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
        "named_registry repeats concept_id SYN-SP01",
    );
}

#[test]
fn a_primary_table_naming_another_subspecies_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    edit_table(&mut synthetic, PRIMARY_TABLE, |text| {
        text.replace("\talter\t1\t0\t", "\tsyntheticus\t1\t0\t")
    });
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "primary_whole:GCA_900000003.1",
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
        entries[0].0 = "/mnt/synthetic/sequences/whole/GCA_900000009.1.fna".into()
    });
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "the skani index of primary_whole names",
    );

    let mut relative = Synthetic::new();
    edit_index(&mut relative, |entries| {
        entries[0].0 = "not-a-reference".into()
    });
    assert_rejected(
        &relative,
        PackageErrorKind::Inconsistent,
        "not an absolute archived path",
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
        "the skani index of primary_whole does not name",
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

/// Set one cell of a table, found by its key column, and re-seal the table, keeping its line ends.
fn edit_cell(synthetic: &mut Synthetic, path: &str, key: &str, column: &str, value: &str) {
    edit_table(synthetic, path, |text| {
        let ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let mut lines: Vec<Vec<String>> = text
            .split(ending)
            .map(|line| line.split('\t').map(str::to_owned).collect())
            .collect();
        let at = lines[0].iter().position(|name| name == column).unwrap();
        let row = lines
            .iter_mut()
            .find(|cells| cells[0] == key)
            .unwrap_or_else(|| panic!("no row {key} in {path}"));
        row[at] = value.to_owned();
        lines
            .iter()
            .map(|cells| cells.join("\t"))
            .collect::<Vec<_>>()
            .join(ending)
    });
}

/// Remove the row whose first cell is `key`, and re-seal the table.
fn remove_row(synthetic: &mut Synthetic, path: &str, key: &str) {
    edit_table(synthetic, path, |text| {
        text.split_inclusive('\n')
            .filter(|line| !line.starts_with(&format!("{key}\t")))
            .collect()
    });
}

#[test]
fn an_accession_in_both_components_is_two_scoped_references() {
    let package = Synthetic::new().verify().expect("the package verifies");

    for accession in ["GCA_900000002.1", "GCA_900000003.1"] {
        let primary = package
            .reference(&format!("primary_whole:{accession}"))
            .expect("a primary reference");
        let member = package
            .reference(&format!("genomovar_chromosome:{accession}"))
            .expect("a definition member");
        assert_eq!(primary.accession, member.accession);
        assert_eq!(primary.sequence_scope, "whole_assembly_as_deposited");
        assert_eq!(member.sequence_scope, "historical_chromosome_record_set");
        assert_ne!(primary.source.sha256, member.source.sha256);
    }
    let counts = package.counts();
    assert_eq!(counts.reference_scope_records, 9);
    assert_eq!(counts.unique_accessions, 7);
    assert_eq!(package.references().count(), 9);
}

#[test]
fn the_cohort_is_not_the_set_of_packaged_references() {
    let package = Synthetic::new().verify().expect("the package verifies");

    // GCA_900000005.1 is a primary reference outside the reporting cohort, and five cohort rows
    // have no packaged reference at all.
    assert!(package.reference("primary_whole:GCA_900000005.1").is_some());
    assert_eq!(package.counts().eligible_cohort_records, 11);
    assert_eq!(package.counts().eligible_species_reference_represented, 7);
    let summary: Vec<_> = package
        .cohort_coverage_summary()
        .iter()
        .map(|(category, count)| (category.as_str(), *count))
        .collect();
    assert_eq!(
        summary,
        [
            ("named_Enterobacter_reference_evidence_absent", 1),
            ("named_Enterobacter_reference_evidence_present", 7),
            ("named_other_genus_context_reference_absent", 1),
            ("restricted_or_out_of_genus_context_reference_absent", 1),
            ("reviewed_unassigned_lineage_reference_absent", 1),
        ]
    );
}

#[test]
fn cohort_membership_flags_must_match_the_packaged_components() {
    for (accession, column, value) in [
        ("GCA_900000002.1", "in_primary140", "False"),
        ("GCA_900000002.1", "in_genomovar173", "False"),
        ("GCA_900000001.1", "in_genomovar173", "True"),
        ("GCA_900000030.1", "in_primary140", "True"),
    ] {
        let mut synthetic = Synthetic::new();
        edit_cell(&mut synthetic, COHORT_TABLE, accession, column, value);
        assert_rejected(&synthetic, PackageErrorKind::Inconsistent, accession);
    }
}

#[test]
fn cohort_evidence_flags_must_follow_whether_the_concept_is_represented() {
    for (accession, column, value) in [
        (
            "GCA_900000031.1",
            "species_reference_evidence_present",
            "True",
        ),
        (
            "GCA_900000030.1",
            "subspecies_reference_evidence_present",
            "False",
        ),
        ("GCA_900000004.1", "validated_automatic_assignment", "True"),
    ] {
        let mut synthetic = Synthetic::new();
        edit_cell(&mut synthetic, COHORT_TABLE, accession, column, value);
        assert_rejected(&synthetic, PackageErrorKind::Inconsistent, accession);
    }
}

#[test]
fn cohort_concepts_must_exist_at_their_rank() {
    for (accession, column, value) in [
        ("GCA_900000004.1", "subspecies_concept_id", "SYN-SP01"),
        ("GCA_900000004.1", "species_concept_id", "SYN-SP09"),
        ("GCA_900000032.1", "lineage_context_id", "SYN-SP01"),
    ] {
        let mut synthetic = Synthetic::new();
        edit_cell(&mut synthetic, COHORT_TABLE, accession, column, value);
        assert_rejected(&synthetic, PackageErrorKind::Inconsistent, accession);
    }
}

#[test]
fn dropping_a_cohort_row_is_inconsistent_with_the_recorded_counts() {
    let mut synthetic = Synthetic::new();
    remove_row(&mut synthetic, COHORT_TABLE, "GCA_900000031.1");
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "the recipe's expected counts differ from the counts the records give",
    );
}

#[test]
fn a_renamed_cohort_category_is_inconsistent_with_the_recorded_summary() {
    let mut synthetic = Synthetic::new();
    edit_cell(
        &mut synthetic,
        COHORT_TABLE,
        "GCA_900000032.1",
        "coverage_category",
        "unassigned",
    );
    assert_rejected(&synthetic, PackageErrorKind::Inconsistent, UNDERIVED);
}

#[test]
fn a_historical_type_crosswalk_that_differs_from_its_table_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    remove_row(&mut synthetic, HISTORICAL_TABLE, "Enterobacter orphanus");
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "historical_type_crosswalk",
    );
}

#[test]
fn an_assignment_overlay_that_differs_from_the_references_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    edit_cell(
        &mut synthetic,
        ASSIGNMENTS_TABLE,
        "GCA_900000005.1",
        "assignment_status",
        "approved_review_treatment",
    );
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "primary_whole:GCA_900000005.1",
    );
}

#[test]
fn a_genomovar_with_another_parent_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    edit_cell(
        &mut synthetic,
        CROSSWALK_TABLE,
        "GV03",
        "parent_concept_id",
        "SYN-SS02",
    );
    assert_rejected(&synthetic, PackageErrorKind::Inconsistent, "GV03");
}

#[test]
fn a_source_manifest_with_a_stale_concept_name_is_inconsistent() {
    let mut synthetic = Synthetic::new();
    edit_source(&mut synthetic, |source| {
        let concept = source["concepts"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|concept| concept["concept_id"] == "SYN-SP02")
            .unwrap();
        concept["display_name"] = json!("Enterobacter fictitius");
    });
    assert_rejected(
        &synthetic,
        PackageErrorKind::Inconsistent,
        "concepts[SYN-SP02].display_name",
    );
}

#[test]
fn a_package_without_a_registry_table_is_rejected_as_missing() {
    let mut synthetic = Synthetic::new();
    synthetic.remove(NAMED_REGISTRY);
    assert_rejected(&synthetic, PackageErrorKind::Missing, NAMED_REGISTRY);
}

#[test]
fn concepts_carry_their_rank_parent_names_and_status() {
    let package = Synthetic::new().verify().expect("the package verifies");

    let ranks = |rank| {
        package
            .concepts()
            .iter()
            .filter(|concept| concept.rank == rank)
            .map(|concept| concept.id.as_str())
            .collect::<Vec<_>>()
    };
    assert_eq!(ranks(Rank::Species), ["SYN-SP01", "SYN-SP02", "SYN-SP03"]);
    assert_eq!(ranks(Rank::Subspecies), ["SYN-SS01", "SYN-SS02"]);
    assert_eq!(ranks(Rank::Lineage), ["SYN-LN01", "SYN-LN02"]);
    assert_eq!(ranks(Rank::Genomovar).len(), 3);

    let alter = package.concept("SYN-SS02").expect("a subspecies");
    assert_eq!(alter.parent_id.as_deref(), Some("SYN-SP01"));
    assert_eq!(alter.display_name, "Enterobacter syntheticus subsp. alter");
    assert_eq!(
        alter.nomenclature_status,
        "approved_project_subspecies_proposal_not_validated_combination"
    );
    assert_eq!(alter.reference_state, "represented");

    let absent = package
        .concept("SYN-SP03")
        .expect("an unrepresented species");
    assert_eq!(absent.parent_id, None);
    assert_eq!(absent.reference_state, "unsupported_no_reference");

    let lineage = package.concept("SYN-LN01").expect("a context lineage");
    assert_eq!(lineage.reference_state, "context_only");
    assert!(package
        .references()
        .all(|reference| reference.species_id != lineage.id
            && reference.subspecies_id.as_deref() != Some(&lineage.id)));
}

#[test]
fn genomovar_concepts_carry_their_versioned_definition() {
    let package = Synthetic::new().verify().expect("the package verifies");

    let genomovar = package
        .concept("ECTX-GV-2026-01-01-v1-GV01")
        .expect("a genomovar");
    assert_eq!(genomovar.parent_id.as_deref(), Some("SYN-SS02"));
    assert_eq!(genomovar.registry_row, None);
    let definition = genomovar.definition.as_ref().expect("a definition");
    assert_eq!(definition.version, "2026-01-01-v1");
    assert_eq!(definition.rule, "synthetic_chromosome_ANI_group_rule");
    assert_eq!(
        definition.member_ids,
        [
            "genomovar_chromosome:GCA_900000002.1",
            "genomovar_chromosome:GCA_900000006.1"
        ]
    );
    assert_eq!(
        definition.representative_id,
        "genomovar_chromosome:GCA_900000006.1"
    );
    assert!(!definition.production_membership_policy_enabled);
    for member in &definition.member_ids {
        let reference = package.reference(member).expect("a packaged member");
        assert_eq!(reference.component_id, "genomovar_chromosome");
    }
}

#[test]
fn references_carry_their_type_and_quality_annotations() {
    let package = Synthetic::new().verify().expect("the package verifies");

    let exception = package
        .reference("primary_whole:GCA_900000002.1")
        .expect("a type anchor");
    assert!(exception.type_anchor);
    assert!(!exception.supplementary_qc_pass);
    assert_eq!(exception.quality_state, "type_QC_exception");
    assert_eq!(exception.species_id, "SYN-SP01");
    assert_eq!(exception.subspecies_id.as_deref(), Some("SYN-SS02"));
    assert_eq!(exception.assignment_status, "retained_formal_type_anchor");

    let diversity = package
        .reference("primary_whole:GCA_900000005.1")
        .expect("a diversity reference");
    assert!(!diversity.type_anchor);
    assert_eq!(diversity.quality_flags, "low_completeness");
    assert_eq!(diversity.subspecies_id, None);
    assert_eq!(package.counts().primary_historical_type_accessions, 3);
}

#[test]
fn context_records_and_historical_types_are_kept_but_never_references() {
    let package = Synthetic::new().verify().expect("the package verifies");

    let context: Vec<_> = package
        .context_records()
        .iter()
        .map(|record| record["accession"].as_str())
        .collect();
    assert_eq!(context, ["GCA_900000020.1", "GCA_900000021.1"]);
    for accession in context {
        assert!(package
            .references()
            .all(|reference| reference.accession != accession));
    }
    let historical: Vec<_> = package
        .historical_types()
        .iter()
        .map(|record| record["historical_taxon_name"].as_str())
        .collect();
    assert!(
        historical.contains(&"Enterobacter orphanus"),
        "{historical:?}"
    );
}

#[test]
fn a_converted_package_records_the_version_1_manifest_it_came_from() {
    let synthetic = Synthetic::new();
    assert_eq!(
        sha256_hex(synthetic.file(CONVERTED_FROM_PATH)),
        CONVERTED_FROM_SHA256
    );
    let package = synthetic.verify().expect("the package verifies");
    assert_eq!(
        package.package().converted_from_sha256.as_deref(),
        Some(CONVERTED_FROM_SHA256)
    );
}

/// An edit to a JSON record, named by what it changes.
type Edit = fn(&mut Value);

#[test]
fn a_conversion_record_that_does_not_describe_this_package_is_inconsistent() {
    let edits: [(&str, Edit); 4] = [
        ("package_id", |v1| {
            v1["package_id"] = json!("another-package")
        }),
        ("assets", |v1| {
            v1["assets"].as_array_mut().unwrap().pop();
        }),
        ("source_manifest", |v1| {
            v1["source_manifest"]["sha256"] = json!("c".repeat(64))
        }),
        ("licence", |v1| v1["licence"] = json!({})),
    ];
    for (field, edit) in edits {
        let mut synthetic = Synthetic::new();
        synthetic.edit_conversion(edit);
        let error = synthetic.verify().expect_err(field);
        assert_eq!(
            error.kind(),
            PackageErrorKind::Inconsistent,
            "{field}: {error}"
        );
        assert!(
            error.to_string().contains("the converted-from manifest"),
            "{field}: {error}"
        );
    }
}

#[test]
fn a_conversion_record_of_another_schema_is_incompatible() {
    let mut synthetic = Synthetic::new();
    synthetic
        .edit_conversion(|v1| v1["schema_version"] = json!("2.0.0-dev-portable-reference-package"));
    assert_rejected(
        &synthetic,
        PackageErrorKind::Incompatible,
        "the converted-from manifest is not schema",
    );
}

#[test]
fn a_damaged_or_absent_conversion_record_is_rejected() {
    let mut altered = Synthetic::new();
    altered.alter(CONVERTED_FROM_PATH);
    assert_rejected(&altered, PackageErrorKind::Altered, CONVERTED_FROM_PATH);

    let mut missing = Synthetic::new();
    missing.remove(CONVERTED_FROM_PATH);
    assert_rejected(&missing, PackageErrorKind::Missing, CONVERTED_FROM_PATH);
}

#[test]
fn a_native_version_2_package_verifies_without_a_conversion_record() {
    let mut synthetic = Synthetic::new();
    synthetic.without_conversion();
    let package = synthetic.verify().expect("a native export verifies");
    assert_eq!(package.package().converted_from_sha256, None);
    assert_eq!(package.counts().reference_scope_records, 9);
}

fn public_flavour() -> Synthetic {
    let mut s = Synthetic::new();
    s.without_conversion();
    edit_source(&mut s, |v| {
        v["schema_version"] = "1.1.0-public-reference-package".into();
        v["release_state"] = "public_reference_export".into();
        v["path_semantics"] = "namespace_labels_not_local_paths".into();
    });
    s.edit_json(EXPORT_PATH, |v| {
        v["local_validation"]["schema_version"] = "1.1.0-public-reference-package".into();
        v["no_external_distribution"] = false.into();
        v["scope"] =
            "frozen_package_transformation_validated_at_export_not_reexecuted_at_load".into();
    });
    s.edit_manifest(|v| {
        v["schema_version"] = "2.1.0-public-portable-reference-package".into();
        v["release_state"] = "public_reference_export".into();
        v["trust_basis"] = "catalogue_pinned_manifest_digest_and_export_attestation".into();
    });
    s
}

#[test]
fn public_flavour_requires_the_entire_profile_and_preserves_scientific_checks() {
    public_flavour()
        .verify()
        .expect("explicit pin accepts the public profile");
    for field in ["schema_version", "release_state", "trust_basis"] {
        let mut s = public_flavour();
        let original: Value = serde_json::from_slice(&Synthetic::new().files()[MANIFEST]).unwrap();
        s.edit_manifest(|v| v[field] = original[field].clone());
        assert!(s.verify().is_err(), "mixed portable {field}");
    }
    for field in ["schema_version", "release_state", "path_semantics"] {
        let mut s = public_flavour();
        let original: Value = serde_json::from_slice(Synthetic::new().file(SOURCE_PATH)).unwrap();
        edit_source(&mut s, |v| v[field] = original[field].clone());
        assert!(s.verify().is_err(), "mixed source {field}");
    }
    for field in ["local_validation", "no_external_distribution", "scope"] {
        let mut s = public_flavour();
        let original: Value = serde_json::from_slice(Synthetic::new().file(EXPORT_PATH)).unwrap();
        s.edit_json(EXPORT_PATH, |v| v[field] = original[field].clone());
        assert!(s.verify().is_err(), "mixed attestation {field}");
    }
    let mut s = public_flavour();
    s.edit_manifest(|v| v["schema_version"] = "2.2.0-unknown".into());
    assert_eq!(
        s.verify().unwrap_err().kind(),
        PackageErrorKind::Incompatible
    );
    let mut s = public_flavour();
    s.alter(PRIMARY_INDEX);
    assert!(s.verify().is_err());
    let mut s = public_flavour();
    edit_source(&mut s, |v| {
        v["positive_calling_policy_enabled"] = true.into()
    });
    assert!(s.verify().is_err());
    Synthetic::new()
        .verify()
        .expect("frozen development flavour still verifies");
}
