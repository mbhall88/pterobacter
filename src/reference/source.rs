//! The source manifest records the components, concepts and references the package was built
//! from. The export attestation says the package passed local validation when it was exported.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::Deserialize;

use super::index;
use super::manifest::Portable;
use super::{inconsistent, json_record, Asset, PackageError, PackageReference, ReferenceComponent};

const SOURCE_SCHEMA: &str = "1.0.0-dev-reference-package";
const EXPORT_SCOPE: &str = "full_local_source_validation_at_export_not_reexecuted_at_load";
const ENGINE: &str = "skani";
const ENGINE_VERSION: &str = "0.3.2";
const DATABASE_FORMAT: &str = "skani-0.3.2-consolidated-bincode1";

/// Each component with the reference role and sequence scope its references must have.
const COMPONENTS: [(&str, &str, &str); 2] = [
    (
        "primary_whole",
        "primary_named_reference_evidence",
        "whole_assembly_as_deposited",
    ),
    (
        "genomovar_chromosome",
        "genomovar_definition_member",
        "historical_chromosome_record_set",
    ),
];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Counts {
    pub(super) context_lineage_concepts: u64,
    pub(super) development_context_records_not_packaged: u64,
    pub(super) eligible_cohort_records: u64,
    pub(super) eligible_species_reference_represented: u64,
    pub(super) genomovar_concepts: u64,
    pub(super) historical_chromosome_definition_references: u64,
    pub(super) named_concepts: u64,
    pub(super) primary_historical_type_accessions: u64,
    pub(super) primary_whole_references: u64,
    pub(super) reference_scope_records: u64,
    pub(super) unique_accessions: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct FileRef {
    pub(super) bytes: u64,
    pub(super) path: String,
    pub(super) sha256: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct Concept {
    pub(super) concept_id: String,
    pub(super) rank: String,
    pub(super) parent_concept_id: Option<String>,
    pub(super) display_name: String,
    pub(super) nomenclature_status: String,
    #[serde(default)]
    pub(super) source_annotation: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub(super) definition_member_ids: Vec<String>,
    #[serde(default)]
    pub(super) definition_version: Option<String>,
    #[serde(default)]
    pub(super) historical_definition_rule: Option<String>,
    #[serde(default)]
    pub(super) representative_reference_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SourceReference {
    pub(super) accession: String,
    pub(super) component_id: String,
    pub(super) reference_id: String,
    pub(super) reference_role: String,
    pub(super) search_label: String,
    pub(super) sequence_scope: String,
    pub(super) source: FileRef,
    pub(super) species_id: String,
    pub(super) subspecies_id: Option<String>,
    pub(super) type_anchor: bool,
    pub(super) quality_flags: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct HistoricalType {
    pub(super) historical_taxon_name: String,
    pub(super) represented_name_bearing_accessions: String,
    pub(super) current_project_species: String,
    pub(super) current_project_stratum: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ContextRecord {
    pub(super) accession: String,
    pub(super) context_disposition: String,
    pub(super) reference_in_proposed_package: String,
}

#[derive(Debug, Deserialize)]
struct SourceComponent {
    component_id: String,
    database_files: Vec<FileRef>,
    database_format: String,
    database_index_entries: Vec<IndexRef>,
    engine: String,
    engine_version: String,
    reference_ids: Vec<String>,
    reference_role: String,
    sequence_scope: String,
    sketch_payload_recomputed: bool,
}

#[derive(Debug, Deserialize)]
struct IndexRef {
    bytes: u64,
    offset: u64,
    path: String,
}

#[derive(Debug, Deserialize)]
struct Recipe {
    package_id: String,
    package_version: String,
    components: BTreeMap<String, RecipeComponent>,
    expected_counts: Counts,
    provenance_files: Vec<FileRef>,
    source_tables: BTreeMap<String, FileRef>,
}

#[derive(Debug, Deserialize)]
struct RecipeComponent {
    build_records: Vec<FileRef>,
    database_files: Vec<FileRef>,
    source_list: FileRef,
}

#[derive(Debug, Deserialize)]
pub(super) struct SourceManifest {
    pub(super) package_id: String,
    pub(super) package_version: String,
    pub(super) release_state: String,
    pub(super) positive_calling_policy_enabled: bool,
    pub(super) counts: Counts,
    pub(super) cohort_coverage_summary: BTreeMap<String, u64>,
    components: Vec<SourceComponent>,
    pub(super) concepts: Vec<Concept>,
    pub(super) references: Vec<SourceReference>,
    pub(super) historical_type_crosswalk: Vec<HistoricalType>,
    pub(super) context_not_packaged: Vec<ContextRecord>,
    recipe: Recipe,
}

/// The export attestation, which records that the package passed local validation.
#[derive(Debug, Deserialize)]
pub(super) struct Export {
    pub(super) local_validation: LocalValidation,
    no_external_distribution: bool,
    no_sequence_or_sketch_recomputation: bool,
    scope: String,
    source_manifest_sha256: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct LocalValidation {
    pub(super) counts: Counts,
    database_bytes: u64,
    positive_calling_policy_enabled: bool,
    schema_version: String,
    valid: bool,
}

impl SourceManifest {
    pub(super) fn parse(bytes: &[u8]) -> Result<Self, PackageError> {
        json_record(bytes, "the source manifest", SOURCE_SCHEMA)
    }

    pub(super) fn check_identity(&self, portable: &Portable) -> Result<(), PackageError> {
        if self.package_id != portable.package_id
            || self.package_version != portable.package_version
            || self.release_state != portable.release_state
            || self.recipe.package_id != portable.package_id
            || self.recipe.package_version != portable.package_version
        {
            return Err(inconsistent(
                "the source manifest names another package or version than the portable manifest",
            ));
        }
        if self.positive_calling_policy_enabled {
            return Err(inconsistent("the source manifest enables positive calling"));
        }
        Ok(())
    }

    pub(super) fn check_attestation(
        &self,
        bytes: &[u8],
        portable: &Portable,
    ) -> Result<Export, PackageError> {
        let export: Export = serde_json::from_slice(bytes).map_err(|error| {
            inconsistent(format!("the export attestation is malformed: {error}"))
        })?;
        let local = &export.local_validation;
        if !local.valid
            || local.positive_calling_policy_enabled
            || local.schema_version != SOURCE_SCHEMA
        {
            return Err(inconsistent(
                "the export attestation does not record a valid local validation",
            ));
        }
        if !export.no_external_distribution
            || !export.no_sequence_or_sketch_recomputation
            || export.scope != EXPORT_SCOPE
        {
            return Err(inconsistent("the export attestation has another scope"));
        }
        if export.source_manifest_sha256 != portable.source_manifest.sha256 {
            return Err(inconsistent(
                "the export attestation is for another source manifest",
            ));
        }
        let database_bytes: u64 = portable
            .assets
            .iter()
            .filter(|asset| asset.roles.iter().any(|role| role.starts_with("database:")))
            .map(|asset| asset.bytes)
            .sum();
        if local.database_bytes != database_bytes {
            return Err(inconsistent(
                "the export attestation counts other database bytes than the manifest lists",
            ));
        }
        Ok(export)
    }

    /// Every listed file must be one the recipe names, once, with the recipe's length and digest.
    pub(super) fn check_recipe(
        &self,
        assets: &[Asset],
        export: &Export,
    ) -> Result<(), PackageError> {
        let recipe = &self.recipe;
        if recipe.expected_counts != self.counts || export.local_validation.counts != self.counts {
            return Err(inconsistent(
                "the recipe or the export attestation counts differ from the source manifest",
            ));
        }
        let mut expected: Vec<&FileRef> = Vec::new();
        for component in recipe.components.values() {
            expected.extend(&component.database_files);
            expected.extend(&component.build_records);
            expected.push(&component.source_list);
        }
        expected.extend(&recipe.provenance_files);
        expected.extend(recipe.source_tables.values());

        let mut named = HashSet::new();
        if expected
            .iter()
            .any(|file| !named.insert(file.path.as_str()))
        {
            return Err(inconsistent("the recipe names a file twice"));
        }
        let mut sources = HashSet::new();
        for asset in assets {
            let Some(source) = asset.source_path.as_deref() else {
                return Err(inconsistent(format!(
                    "{} records no source path",
                    asset.path
                )));
            };
            if !sources.insert(source) {
                return Err(inconsistent(format!("{source} is listed twice")));
            }
        }
        if assets.len() != expected.len() {
            return Err(inconsistent(format!(
                "the manifest lists {} files but the recipe names {}",
                assets.len(),
                expected.len()
            )));
        }
        for file in expected {
            let asset = asset_for(assets, &file.path)?;
            if asset.bytes != file.bytes || asset.sha256 != file.sha256 {
                return Err(inconsistent(format!(
                    "{} differs from the recipe",
                    asset.path
                )));
            }
        }
        Ok(())
    }

    pub(super) fn concept(&self, id: &str) -> Option<&Concept> {
        self.concepts
            .iter()
            .find(|concept| concept.concept_id == id)
    }

    pub(super) fn components(
        &self,
        assets: &[Asset],
        held: &BTreeMap<&str, Vec<u8>>,
    ) -> Result<Vec<ReferenceComponent>, PackageError> {
        self.check_references()?;
        if self.components.len() != COMPONENTS.len() {
            return Err(inconsistent(
                "the source manifest does not describe exactly the two components",
            ));
        }
        COMPONENTS
            .iter()
            .map(|&(id, role, scope)| self.component(id, role, scope, assets, held))
            .collect()
    }

    fn check_references(&self) -> Result<(), PackageError> {
        let mut ids = HashSet::new();
        let mut accessions = HashSet::new();
        let mut sources = HashSet::new();
        for reference in &self.references {
            if !ids.insert(reference.reference_id.as_str())
                || !accessions.insert(reference.accession.as_str())
                || !sources.insert(reference.source.path.as_str())
            {
                return Err(inconsistent(format!(
                    "reference {} is repeated",
                    reference.reference_id
                )));
            }
            if self
                .concept(&reference.species_id)
                .is_none_or(|c| c.rank != "species")
            {
                return Err(inconsistent(format!(
                    "reference {} names no species concept",
                    reference.reference_id
                )));
            }
            if let Some(subspecies) = &reference.subspecies_id {
                let found = self.concept(subspecies).is_some_and(|concept| {
                    concept.rank == "subspecies"
                        && concept.parent_concept_id.as_deref()
                            == Some(reference.species_id.as_str())
                });
                if !found {
                    return Err(inconsistent(format!(
                        "reference {} names no subspecies of its species",
                        reference.reference_id
                    )));
                }
            }
        }
        Ok(())
    }

    fn component(
        &self,
        id: &str,
        role: &str,
        scope: &str,
        assets: &[Asset],
        held: &BTreeMap<&str, Vec<u8>>,
    ) -> Result<ReferenceComponent, PackageError> {
        let source = self
            .components
            .iter()
            .find(|component| component.component_id == id)
            .ok_or_else(|| inconsistent(format!("the source manifest has no component {id}")))?;
        if source.engine != ENGINE
            || source.engine_version != ENGINE_VERSION
            || source.database_format != DATABASE_FORMAT
            || source.reference_role != role
            || source.sequence_scope != scope
            || source.sketch_payload_recomputed
        {
            return Err(inconsistent(format!(
                "component {id} does not match the package format"
            )));
        }
        let recipe = self
            .recipe
            .components
            .get(id)
            .ok_or_else(|| inconsistent(format!("the recipe has no component {id}")))?;
        if source.database_files != recipe.database_files {
            return Err(inconsistent(format!(
                "component {id} names other database files than the recipe"
            )));
        }

        let members: Vec<&SourceReference> = self
            .references
            .iter()
            .filter(|reference| reference.component_id == id)
            .collect();
        for reference in &members {
            if reference.reference_role != role
                || reference.sequence_scope != scope
                || reference.reference_id != format!("{id}:{}", reference.accession)
                || reference.search_label != reference.source.path
            {
                return Err(inconsistent(format!(
                    "reference {} does not match component {id}",
                    reference.reference_id
                )));
            }
        }
        let mut listed: Vec<&str> = source.reference_ids.iter().map(String::as_str).collect();
        let mut expected: Vec<&str> = members.iter().map(|r| r.reference_id.as_str()).collect();
        listed.sort_unstable();
        expected.sort_unstable();
        if listed != expected {
            return Err(inconsistent(format!(
                "component {id} lists other reference ids than its references"
            )));
        }

        let mut databases = BTreeMap::new();
        for file in &recipe.database_files {
            let asset = asset_for(assets, &file.path)?;
            databases.insert(file_name(&asset.path).to_string(), asset.clone());
        }
        let index_asset = databases
            .get("index.db")
            .ok_or_else(|| inconsistent(format!("component {id} has no index.db")))?;
        let sketches = databases
            .get("sketches.db")
            .ok_or_else(|| inconsistent(format!("component {id} has no sketches.db")))?;
        let index_bytes = held
            .get(index_asset.path.as_str())
            .ok_or_else(|| inconsistent(format!("{} was not read", index_asset.path)))?;
        let entries = index::parse(index_bytes, sketches.bytes)?;
        let labels: BTreeSet<&str> = members.iter().map(|r| r.source.path.as_str()).collect();
        index::check_membership(&entries, &labels)?;
        if entries.len() != source.database_index_entries.len() {
            return Err(inconsistent(format!(
                "the index of component {id} lists another number of sequences than the source manifest"
            )));
        }
        for (entry, listed) in entries.iter().zip(&source.database_index_entries) {
            if entry.label != listed.path
                || entry.offset != listed.offset
                || entry.bytes != listed.bytes
            {
                return Err(inconsistent(format!(
                    "the index of component {id} differs from the source manifest"
                )));
            }
        }

        let list = asset_for(assets, &recipe.source_list.path)?;
        let list_bytes = held
            .get(list.path.as_str())
            .ok_or_else(|| inconsistent(format!("{} was not read", list.path)))?;
        let text = std::str::from_utf8(list_bytes)
            .map_err(|_| inconsistent(format!("{} is not UTF-8", list.path)))?;
        let mut lines: Vec<&str> = text.lines().collect();
        let mut paths: Vec<&str> = members.iter().map(|r| r.source.path.as_str()).collect();
        lines.sort_unstable();
        paths.sort_unstable();
        if lines != paths {
            return Err(inconsistent(format!(
                "{} does not list the sequences of component {id}",
                list.path
            )));
        }

        let builds = recipe
            .build_records
            .iter()
            .map(|file| asset_for(assets, &file.path).cloned())
            .collect::<Result<Vec<_>, _>>()?;
        let references = members
            .iter()
            .map(|reference| PackageReference {
                reference_id: reference.reference_id.clone(),
                accession: reference.accession.clone(),
                species_id: reference.species_id.clone(),
                subspecies_id: reference.subspecies_id.clone(),
            })
            .collect();
        Ok(ReferenceComponent {
            id: id.to_string(),
            reference_role: role.to_string(),
            sequence_scope: scope.to_string(),
            databases,
            builds,
            references,
        })
    }
}

fn asset_for<'a>(assets: &'a [Asset], source_path: &str) -> Result<&'a Asset, PackageError> {
    assets
        .iter()
        .find(|asset| asset.source_path.as_deref() == Some(source_path))
        .ok_or_else(|| inconsistent(format!("the manifest lists no file for {source_path}")))
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}
