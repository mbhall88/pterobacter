//! The checked metadata a verified package exposes: concepts with their parents and genomovar
//! definitions, every reference scope record with its type and QC annotations, both components,
//! the historical type crosswalk, the context records kept out of the package, and the counts.
//!
//! These are built only after the source manifest has been shown to equal the derivation of its
//! tables, and they cannot be constructed outside this crate, so holding one means it came from a
//! verified package. Archived paths are kept as opaque provenance labels, never as files to open.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use super::{inconsistent, Asset, PackageError};

/// The rank of a concept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rank {
    Species,
    Subspecies,
    /// A reviewed context lineage. It has no references and is never a search target.
    Lineage,
    /// An informal chromosome ANI group defined by historical chromosome records.
    Genomovar,
}

impl Rank {
    pub fn as_str(self) -> &'static str {
        match self {
            Rank::Species => "species",
            Rank::Subspecies => "subspecies",
            Rank::Lineage => "lineage",
            Rank::Genomovar => "genomovar",
        }
    }
}

/// A named, context or genomovar concept from the package's registries.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Concept {
    pub id: String,
    pub rank: Rank,
    /// The species of a subspecies, or the subspecies of a genomovar. Species and lineages have none.
    pub parent_id: Option<String>,
    pub display_name: String,
    pub nomenclature_status: String,
    pub identifier_status: String,
    /// `represented`, `unsupported_no_reference`, `context_only` or `represented_definition`.
    pub reference_state: String,
    pub sampling_state: String,
    /// Recorded as false for every concept in a development package; not a calling policy.
    pub validated_supported_caller_rank: bool,
    /// The whole registry row the concept came from. Genomovars come from the group tables and have none.
    pub registry_row: Option<BTreeMap<String, String>>,
    pub definition: Option<GenomovarDefinition>,
}

/// How a genomovar was defined. Membership is historical provenance, not a membership policy.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GenomovarDefinition {
    pub version: String,
    pub rule: String,
    pub engine: String,
    pub sequence_scope: String,
    /// Reference IDs in the `genomovar_chromosome` component, in ID order.
    pub member_ids: Vec<String>,
    pub representative_id: String,
    pub representative_role: String,
    pub production_membership_policy_enabled: bool,
}

/// A pinned archived file: its original path is a label, not a location to read.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ArchivedFile {
    pub label: String,
    pub sha256: String,
    pub bytes: u64,
}

/// One reference scope record. An accession may have one record in each component.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PackageReference {
    /// `component_id:accession`.
    pub reference_id: String,
    pub accession: String,
    pub component_id: String,
    pub reference_role: String,
    pub sequence_scope: String,
    pub species_id: String,
    pub subspecies_id: Option<String>,
    /// The subspecies when there is one, otherwise the species.
    pub stratum_concept_id: String,
    pub type_anchor: bool,
    pub supplementary_qc_pass: bool,
    /// `source_QC_pass`, `source_QC_fail` or `type_QC_exception`.
    pub quality_state: String,
    pub quality_flags: String,
    pub assignment_status: String,
    pub source_selection_reason: String,
    /// The exact label the skani index uses for this sequence.
    pub search_label: String,
    pub source: ArchivedFile,
    pub historical_origin_label: Option<String>,
    pub historical_source_basis: Option<String>,
    pub historical_origin_validation: String,
}

/// One skani index entry of a component.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct IndexEntry {
    pub label: String,
    pub offset: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ReferenceComponent {
    pub id: String,
    pub reference_role: String,
    pub sequence_scope: String,
    pub engine: String,
    pub engine_version: String,
    pub database_format: String,
    /// Keyed by file name: `index.db`, `markers.bin` and `sketches.db`.
    pub databases: BTreeMap<String, Asset>,
    pub builds: Vec<Asset>,
    pub index_entries: Vec<IndexEntry>,
    /// In reference ID order.
    pub references: Vec<PackageReference>,
}

impl ReferenceComponent {
    pub fn reference(&self, id: &str) -> Option<&PackageReference> {
        self.references
            .binary_search_by(|reference| reference.reference_id.as_str().cmp(id))
            .ok()
            .map(|at| &self.references[at])
    }
}

/// The counts the package records, each of which has been derived again from its records.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct PackageCounts {
    pub primary_whole_references: u64,
    pub historical_chromosome_definition_references: u64,
    pub unique_accessions: u64,
    pub reference_scope_records: u64,
    pub named_concepts: u64,
    pub context_lineage_concepts: u64,
    pub genomovar_concepts: u64,
    pub development_context_records_not_packaged: u64,
    pub eligible_cohort_records: u64,
    pub eligible_species_reference_represented: u64,
    pub primary_historical_type_accessions: u64,
}

/// Everything the verified package says about its references and concepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Metadata {
    pub(super) concepts: Vec<Concept>,
    pub(super) components: Vec<ReferenceComponent>,
    pub(super) historical_types: Vec<BTreeMap<String, String>>,
    pub(super) context_records: Vec<BTreeMap<String, String>>,
    pub(super) cohort_coverage_summary: BTreeMap<String, u64>,
    pub(super) counts: PackageCounts,
}

#[derive(Deserialize)]
struct RawConcept {
    concept_id: String,
    rank: Rank,
    parent_concept_id: Option<String>,
    display_name: String,
    nomenclature_status: String,
    identifier_status: String,
    reference_state: String,
    sampling_state: String,
    validated_supported_caller_rank: bool,
    #[serde(default)]
    source_annotation: Option<BTreeMap<String, String>>,
    #[serde(default)]
    definition_version: Option<String>,
    #[serde(default)]
    historical_definition_rule: Option<String>,
    #[serde(default)]
    historical_definition_engine: Option<String>,
    #[serde(default)]
    definition_sequence_scope: Option<String>,
    #[serde(default)]
    definition_member_ids: Option<Vec<String>>,
    #[serde(default)]
    representative_reference_id: Option<String>,
    #[serde(default)]
    representative_role: Option<String>,
    #[serde(default)]
    production_membership_policy_enabled: Option<bool>,
}

#[derive(Deserialize)]
struct RawReference {
    reference_id: String,
    accession: String,
    component_id: String,
    reference_role: String,
    sequence_scope: String,
    species_id: String,
    subspecies_id: Option<String>,
    stratum_concept_id: String,
    type_anchor: bool,
    supplementary_qc_pass: bool,
    quality_state: String,
    quality_flags: String,
    assignment_status: String,
    source_selection_reason: String,
    search_label: String,
    source: RawFile,
    historical_origin_path: Option<String>,
    historical_source_basis: Option<String>,
    historical_origin_validation: String,
}

#[derive(Deserialize)]
struct RawFile {
    path: String,
    sha256: String,
    bytes: u64,
}

#[derive(Deserialize)]
struct RawComponent {
    component_id: String,
    reference_role: String,
    sequence_scope: String,
    engine: String,
    engine_version: String,
    database_format: String,
    database_index_entries: Vec<RawEntry>,
    database_files: Vec<RawFile>,
}

#[derive(Deserialize)]
struct RawEntry {
    path: String,
    offset: u64,
    bytes: u64,
}

#[derive(Deserialize)]
struct RawSource {
    concepts: Vec<RawConcept>,
    references: Vec<RawReference>,
    components: Vec<RawComponent>,
    historical_type_crosswalk: Vec<BTreeMap<String, String>>,
    context_not_packaged: Vec<BTreeMap<String, String>>,
    cohort_coverage_summary: BTreeMap<String, u64>,
    counts: PackageCounts,
    recipe: RawRecipe,
}

#[derive(Deserialize)]
struct RawRecipe {
    components: BTreeMap<String, RawRecipeComponent>,
}

#[derive(Deserialize)]
struct RawRecipeComponent {
    build_records: Vec<RawFile>,
}

impl Metadata {
    /// Read the typed view of a source manifest that has already passed the derivation check.
    pub(super) fn from_checked(source: &Value, assets: &[Asset]) -> Result<Self, PackageError> {
        let raw: RawSource = serde_json::from_value(source.clone())
            .map_err(|error| inconsistent(format!("the source manifest is malformed: {error}")))?;
        let concepts = raw.concepts.into_iter().map(concept).collect();
        let mut references: Vec<PackageReference> =
            raw.references.into_iter().map(reference).collect();
        references.sort_by(|a, b| a.reference_id.cmp(&b.reference_id));

        let copy_of = |archived: &str| {
            assets
                .iter()
                .find(|asset| asset.source_path == archived)
                .cloned()
                .ok_or_else(|| inconsistent(format!("the manifest lists no copy of {archived}")))
        };
        let mut components = Vec::new();
        for component in raw.components {
            let mut databases = BTreeMap::new();
            for file in &component.database_files {
                let asset = copy_of(&file.path)?;
                let name = asset
                    .path
                    .rsplit('/')
                    .next()
                    .unwrap_or_default()
                    .to_string();
                databases.insert(name, asset);
            }
            let builds = raw
                .recipe
                .components
                .get(&component.component_id)
                .map(|recipe| {
                    recipe
                        .build_records
                        .iter()
                        .map(|file| copy_of(&file.path))
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            components.push(ReferenceComponent {
                references: references
                    .iter()
                    .filter(|r| r.component_id == component.component_id)
                    .cloned()
                    .collect(),
                id: component.component_id,
                reference_role: component.reference_role,
                sequence_scope: component.sequence_scope,
                engine: component.engine,
                engine_version: component.engine_version,
                database_format: component.database_format,
                databases,
                builds,
                index_entries: component
                    .database_index_entries
                    .into_iter()
                    .map(|entry| IndexEntry {
                        label: entry.path,
                        offset: entry.offset,
                        bytes: entry.bytes,
                    })
                    .collect(),
            });
        }
        Ok(Self {
            concepts,
            components,
            historical_types: raw.historical_type_crosswalk,
            context_records: raw.context_not_packaged,
            cohort_coverage_summary: raw.cohort_coverage_summary,
            counts: raw.counts,
        })
    }
}

fn concept(raw: RawConcept) -> Concept {
    let definition = match (raw.rank, raw.definition_member_ids) {
        (Rank::Genomovar, Some(member_ids)) => Some(GenomovarDefinition {
            version: raw.definition_version.unwrap_or_default(),
            rule: raw.historical_definition_rule.unwrap_or_default(),
            engine: raw.historical_definition_engine.unwrap_or_default(),
            sequence_scope: raw.definition_sequence_scope.unwrap_or_default(),
            member_ids,
            representative_id: raw.representative_reference_id.unwrap_or_default(),
            representative_role: raw.representative_role.unwrap_or_default(),
            production_membership_policy_enabled: raw
                .production_membership_policy_enabled
                .unwrap_or_default(),
        }),
        _ => None,
    };
    Concept {
        id: raw.concept_id,
        rank: raw.rank,
        parent_id: raw.parent_concept_id,
        display_name: raw.display_name,
        nomenclature_status: raw.nomenclature_status,
        identifier_status: raw.identifier_status,
        reference_state: raw.reference_state,
        sampling_state: raw.sampling_state,
        validated_supported_caller_rank: raw.validated_supported_caller_rank,
        registry_row: raw.source_annotation,
        definition,
    }
}

fn reference(raw: RawReference) -> PackageReference {
    PackageReference {
        reference_id: raw.reference_id,
        accession: raw.accession,
        component_id: raw.component_id,
        reference_role: raw.reference_role,
        sequence_scope: raw.sequence_scope,
        species_id: raw.species_id,
        subspecies_id: raw.subspecies_id,
        stratum_concept_id: raw.stratum_concept_id,
        type_anchor: raw.type_anchor,
        supplementary_qc_pass: raw.supplementary_qc_pass,
        quality_state: raw.quality_state,
        quality_flags: raw.quality_flags,
        assignment_status: raw.assignment_status,
        source_selection_reason: raw.source_selection_reason,
        search_label: raw.search_label,
        source: ArchivedFile {
            label: raw.source.path,
            sha256: raw.source.sha256,
            bytes: raw.source.bytes,
        },
        historical_origin_label: raw.historical_origin_path,
        historical_source_basis: raw.historical_source_basis,
        historical_origin_validation: raw.historical_origin_validation,
    }
}
