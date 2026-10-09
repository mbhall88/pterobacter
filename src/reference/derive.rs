//! The source manifest is a deterministic derivation of the packaged tables, the recipe and the
//! skani indexes. This module repeats that derivation from the copied files and requires the
//! source manifest to equal it, so no record can disagree with the registry it came from.
//!
//! The rules follow the research builder that wrote the frozen package. Only one value cannot be
//! derived without the original sequence files: the byte length of each archived sequence, which
//! is taken from the manifest as provenance. Archived paths are compared as text and never opened.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Map, Value};

use super::index::{self, Entry};
use super::tables::{cell, Row, Table, Tables};
use super::{inconsistent, is_sha256, Asset, PackageError};

pub(super) const SOURCE_SCHEMA: &str = "1.0.0-dev-reference-package";
pub(super) const RELEASE_STATE: &str = "development_proposal_not_distributable";
pub(super) const ENGINE: &str = "skani";
pub(super) const ENGINE_VERSION: &str = "0.3.2";
pub(super) const DATABASE_FORMAT: &str = "skani-0.3.2-consolidated-bincode1";
pub(super) const PRIMARY: &str = "primary_whole";
pub(super) const GENOMOVAR: &str = "genomovar_chromosome";
const DATABASE_FILES: [&str; 3] = ["index.db", "markers.bin", "sketches.db"];
const IDENTIFIER_STATUS: &str = "stable_id_proposal_not_adopted_registry";
const RESTRICTED: &str = "no_named_classifier_reference";

/// Each component with the reference role and sequence scope of its references, in the order the
/// builder visits them.
pub(super) const COMPONENTS: [(&str, &str, &str); 2] = [
    (
        PRIMARY,
        "primary_named_reference_evidence",
        "whole_assembly_as_deposited",
    ),
    (
        GENOMOVAR,
        "genomovar_definition_member",
        "historical_chromosome_record_set",
    ),
];

/// Require `source` to be exactly the manifest the builder would write for these files.
pub(super) fn check(
    source: &Value,
    tables: &Tables,
    assets: &[Asset],
    held: &BTreeMap<&str, Vec<u8>>,
) -> Result<(), PackageError> {
    let expected = Derivation::new(source, tables, assets, held)?.manifest()?;
    match first_difference(&expected, source, "source manifest") {
        None => Ok(()),
        Some(place) => Err(inconsistent(format!(
            "{place} differs from the record the packaged tables derive"
        ))),
    }
}

struct Concept {
    id: String,
    rank: String,
    parent: Option<String>,
    name: String,
    represented: bool,
    value: Value,
}

struct Reference {
    id: String,
    accession: String,
    component: &'static str,
    species: String,
    subspecies: Option<String>,
    stratum: String,
    type_anchor: bool,
    quality_state: &'static str,
    label: String,
    value: Value,
}

struct Derivation<'a> {
    source: &'a Value,
    tables: &'a Tables,
    assets: &'a [Asset],
    held: &'a BTreeMap<&'a str, Vec<u8>>,
    recipe: &'a Map<String, Value>,
    /// Every pinned file the builder would have checked, by archived path.
    checked: BTreeMap<String, Value>,
}

impl<'a> Derivation<'a> {
    fn new(
        source: &'a Value,
        tables: &'a Tables,
        assets: &'a [Asset],
        held: &'a BTreeMap<&'a str, Vec<u8>>,
    ) -> Result<Self, PackageError> {
        let recipe = source
            .get("recipe")
            .and_then(Value::as_object)
            .ok_or_else(|| inconsistent("the source manifest has no recipe"))?;
        Ok(Self {
            source,
            tables,
            assets,
            held,
            recipe,
            checked: BTreeMap::new(),
        })
    }

    fn manifest(mut self) -> Result<Value, PackageError> {
        let recipe = self.recipe;
        expect_keys(
            recipe,
            &[
                "components",
                "expected_counts",
                "package_id",
                "package_version",
                "provenance_files",
                "source_tables",
            ],
            "the recipe",
        )?;
        let package_id = identifier(&recipe["package_id"], "package_id")?;
        let package_version = identifier(&recipe["package_version"], "package_version")?;
        let source_tables = object(&recipe["source_tables"], "the recipe source tables")?;
        let names: BTreeSet<&str> = super::tables::TABLES.iter().map(|t| t.0).collect();
        if source_tables
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != names
        {
            return Err(inconsistent("the recipe names other source tables"));
        }
        for name in &names {
            self.pin(&source_tables[*name])?;
        }
        let provenance = array(&recipe["provenance_files"], "the recipe provenance files")?;
        if provenance.is_empty() {
            return Err(inconsistent("the recipe has no provenance records"));
        }
        for record in provenance {
            self.pin(record)?;
        }

        let concepts = self.registry_concepts()?;
        let references = self.references(&concepts)?;
        self.check_named_counts(&concepts, &references)?;
        let genomovars = self.genomovars(&concepts)?;
        let mut ids = BTreeSet::new();
        for id in concepts
            .iter()
            .map(|c| c.id.as_str())
            .chain(genomovars.iter().map(|g| g.id.as_str()))
        {
            if !ids.insert(id) {
                return Err(inconsistent(format!("concept {id} is defined twice")));
            }
        }
        let historical = self.historical_types(&concepts, &references)?;
        let context = self.context()?;
        let summary = self.cohort(&concepts)?;
        let components = self.components(&references)?;
        let counts = self.counts(&references)?;
        if recipe["expected_counts"] != counts {
            return Err(inconsistent(
                "the recipe's expected counts differ from the counts the records give",
            ));
        }

        let mut all_concepts: Vec<&Concept> = concepts.iter().chain(&genomovars).collect();
        all_concepts.sort_by(|a, b| a.id.cmp(&b.id));
        let mut sorted_references: Vec<&Reference> = references.iter().collect();
        sorted_references.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(json!({
            "schema_version": SOURCE_SCHEMA,
            "package_id": package_id,
            "package_version": package_version,
            "release_state": RELEASE_STATE,
            "positive_calling_policy_enabled": false,
            "path_semantics": "absolute_existing_local_sources_no_relocation_or_sequence_copy",
            "integrity_scope": "all_pins_and_packaged_sequence_hashes_exact_source_joins_and_database_index_membership",
            "provenance_limit": "hashes_pin_reviewed_local_records_not_authenticated_origin_or_recomputed_sketches",
            "recipe": Value::Object(recipe.clone()),
            "counts": counts,
            "concepts": all_concepts.iter().map(|c| c.value.clone()).collect::<Vec<_>>(),
            "references": sorted_references.iter().map(|r| r.value.clone()).collect::<Vec<_>>(),
            "components": components,
            "historical_type_crosswalk": historical,
            "context_not_packaged": context,
            "cohort_coverage_summary": summary,
            "verified_artifacts": self.checked.into_values().collect::<Vec<_>>(),
        }))
    }

    fn table(&self, name: &str) -> &'a Table {
        self.tables.get(name)
    }

    /// Record a pinned file as checked. The same path must always carry the same pin.
    fn pin(&mut self, record: &Value) -> Result<(), PackageError> {
        let path = file_record(record)?;
        match self.checked.get(path) {
            Some(previous) if previous != record => Err(inconsistent(format!(
                "{path} is pinned with two different digests or lengths"
            ))),
            Some(_) => Ok(()),
            None => {
                self.checked.insert(path.to_string(), record.clone());
                Ok(())
            }
        }
    }

    fn registry_concepts(&self) -> Result<Vec<Concept>, PackageError> {
        let mut concepts = Vec::new();
        for (table, name_column, ranks) in [
            (
                "named_registry",
                "project_name",
                &["species", "subspecies"][..],
            ),
            ("lineage_registry", "project_label", &["lineage"][..]),
        ] {
            let table = self.table(table);
            for row in &table.rows {
                let id = cell(row, "concept_id");
                let rank = cell(row, "rank");
                if !ranks.contains(&rank) {
                    return Err(inconsistent(format!(
                        "{} gives {id} the rank {rank}",
                        table.name
                    )));
                }
                if cell(row, "validated_supported_caller_rank") != "False" {
                    return Err(inconsistent(format!(
                        "{} claims a validated caller rank for {id}",
                        table.name
                    )));
                }
                let count = number(row, "primary140_reference_count", table.name)?;
                if cell(row, "represented_reference_evidence") != python_bool(count > 0) {
                    return Err(inconsistent(format!(
                        "{} gives {id} a representation state its reference count does not",
                        table.name
                    )));
                }
                if rank == "lineage"
                    && (count != 0 || cell(row, "genomovar173_reference_count") != "0")
                {
                    return Err(inconsistent(format!(
                        "context lineage {id} is given references"
                    )));
                }
                let parent = row
                    .get("parent_concept_id")
                    .filter(|parent| !parent.is_empty())
                    .cloned();
                let name = cell(row, name_column).to_string();
                let reference_state = match (count, rank) {
                    (0, "lineage") => "context_only",
                    (0, _) => "unsupported_no_reference",
                    _ => "represented",
                };
                let sampling_state = match count {
                    0 => "absent",
                    1 => "single_reference",
                    _ => "multiple_development_references",
                };
                concepts.push(Concept {
                    id: id.to_string(),
                    rank: rank.to_string(),
                    parent: parent.clone(),
                    name: name.clone(),
                    represented: count > 0,
                    value: json!({
                        "concept_id": id,
                        "rank": rank,
                        "parent_concept_id": parent,
                        "display_name": name,
                        "identifier_status": IDENTIFIER_STATUS,
                        "nomenclature_status": cell(row, "nomenclature_status"),
                        "reference_state": reference_state,
                        "sampling_state": sampling_state,
                        "validated_supported_caller_rank": false,
                        "source_annotation": row,
                    }),
                });
            }
        }
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        for concept in &concepts {
            if concept.id.is_empty() || !ids.insert(concept.id.as_str()) {
                return Err(inconsistent(format!(
                    "the registries repeat concept {}",
                    concept.id
                )));
            }
            if concept.name.is_empty()
                || concept.name.trim() != concept.name
                || !names.insert(concept.name.as_str())
            {
                return Err(inconsistent(format!(
                    "the registries repeat or pad the name {}",
                    concept.name
                )));
            }
        }
        for concept in &concepts {
            let parent_rank = concept
                .parent
                .as_deref()
                .and_then(|id| concepts.iter().find(|c| c.id == id))
                .map(|parent| parent.rank.as_str());
            let valid = match concept.rank.as_str() {
                "subspecies" => parent_rank == Some("species"),
                _ => concept.parent.is_none(),
            };
            if !valid {
                return Err(inconsistent(format!(
                    "concept {} has an undefined or wrong parent",
                    concept.id
                )));
            }
        }
        Ok(concepts)
    }

    fn references(&mut self, concepts: &[Concept]) -> Result<Vec<Reference>, PackageError> {
        let by_name: BTreeMap<&str, &Concept> =
            concepts.iter().map(|c| (c.name.as_str(), c)).collect();
        let by_id: BTreeMap<&str, &Concept> = concepts.iter().map(|c| (c.id.as_str(), c)).collect();
        let assignments = self.table("assignments").by_key();
        let definitions = self.table("definition_references");
        let memberships = self.table("definition_memberships").by_key();
        if definitions.keys() != memberships.keys().copied().collect() {
            return Err(inconsistent(
                "the definition references and memberships name other accessions",
            ));
        }
        let groups = self.table("definition_groups").keys();
        let crosswalk = self.table("genomovar_crosswalk").keys();
        let defined: BTreeSet<&str> = definitions
            .rows
            .iter()
            .map(|row| cell(row, "genomovar_id"))
            .collect();
        if groups != crosswalk || groups != defined {
            return Err(inconsistent(
                "the genomovar group tables name other groups than the definition references",
            ));
        }

        let mut references = Vec::new();
        for (component, role, scope) in COMPONENTS {
            let table = if component == PRIMARY {
                self.table("primary_references")
            } else {
                definitions
            };
            for row in &table.rows {
                let accession = cell(row, "accession");
                let owner = format!("{component}:{accession}");
                if cell(row, "partition") != "development" {
                    return Err(inconsistent(format!(
                        "{owner} is not a development reference"
                    )));
                }
                let concept = by_name
                    .get(cell(row, "group"))
                    .filter(|c| matches!(c.rank.as_str(), "species" | "subspecies"))
                    .ok_or_else(|| {
                        inconsistent(format!("{owner} names no species or subspecies concept"))
                    })?;
                let species = if concept.rank == "species" {
                    *concept
                } else {
                    by_id[concept.parent.as_deref().unwrap_or_default()]
                };
                let overlay = assignments.get(accession).ok_or_else(|| {
                    inconsistent(format!("{owner} has no row in the assignments table"))
                })?;
                if cell(overlay, "project_species") != species.name {
                    return Err(inconsistent(format!(
                        "{owner} has another species than its assignment row"
                    )));
                }
                let subspecies = if concept.rank == "subspecies" {
                    concept.name.as_str()
                } else {
                    ""
                };
                let epithet = cell(overlay, "project_subspecies");
                let expected = if epithet.is_empty() {
                    String::new()
                } else {
                    format!("{} subsp. {epithet}", species.name)
                };
                if subspecies != expected {
                    return Err(inconsistent(format!(
                        "{owner} has another subspecies than its assignment row"
                    )));
                }
                if cell(overlay, "usage_restrictions").contains(RESTRICTED) {
                    return Err(inconsistent(format!(
                        "{owner} is restricted from use as a named reference"
                    )));
                }
                let (path, digest, is_type) = if component == PRIMARY {
                    if cell(row, "project_species") != species.name
                        || cell(row, "named_target") != "1"
                    {
                        return Err(inconsistent(format!(
                            "{owner} is not the named target its row states"
                        )));
                    }
                    if cell(row, "project_subspecies") != epithet {
                        return Err(inconsistent(format!(
                            "{owner} has another subspecies than its assignment row"
                        )));
                    }
                    (
                        cell(row, "whole_fasta"),
                        cell(row, "whole_sha256"),
                        cell(row, "type_anchor"),
                    )
                } else {
                    if concept.rank != "subspecies" {
                        return Err(inconsistent(format!(
                            "{owner} defines a genomovar without a subspecies parent"
                        )));
                    }
                    let member = memberships[accession];
                    if cell(member, "project_species") != species.name
                        || cell(member, "project_subspecies_proposal") != subspecies
                    {
                        return Err(inconsistent(format!(
                            "{owner} has other taxa than its definition membership"
                        )));
                    }
                    for (own, other) in [
                        ("genomovar_id", "genomovar_id"),
                        ("definition_version", "genomovar_version"),
                        ("definition_rule", "genomovar_rule"),
                    ] {
                        let value = cell(row, own);
                        if cell(member, other) != value || cell(overlay, other) != value {
                            return Err(inconsistent(format!(
                                "{owner} has another {own} than its membership or assignment row"
                            )));
                        }
                    }
                    (
                        cell(row, "chromosome_fasta"),
                        cell(row, "chromosome_sha256"),
                        cell(row, "is_type"),
                    )
                };
                let qc = cell(row, "qc_pass");
                if !matches!(is_type, "0" | "1") || !matches!(qc, "0" | "1") {
                    return Err(inconsistent(format!(
                        "{owner} has an invalid type or QC flag"
                    )));
                }
                let (type_anchor, qc_pass) = (is_type == "1", qc == "1");
                let quality_state = match (type_anchor, qc_pass) {
                    (true, false) => "type_QC_exception",
                    (_, true) => "source_QC_pass",
                    (false, false) => "source_QC_fail",
                };
                let sequence = self.sequence(&owner, path, digest)?;
                let subspecies_id = (concept.rank == "subspecies").then(|| concept.id.clone());
                let optional = |column: &str| {
                    table
                        .has_column(column)
                        .then(|| cell(row, column).to_string())
                };
                references.push(Reference {
                    id: owner.clone(),
                    accession: accession.to_string(),
                    component,
                    species: species.id.clone(),
                    subspecies: subspecies_id.clone(),
                    stratum: concept.id.clone(),
                    type_anchor,
                    quality_state,
                    label: path.to_string(),
                    value: json!({
                        "reference_id": owner,
                        "accession": accession,
                        "component_id": component,
                        "reference_role": role,
                        "sequence_scope": scope,
                        "species_id": species.id,
                        "subspecies_id": subspecies_id,
                        "stratum_concept_id": concept.id,
                        "source": sequence,
                        "search_label": path,
                        "type_anchor": type_anchor,
                        "supplementary_qc_pass": qc_pass,
                        "quality_flags": cell(row, "quality_flags"),
                        "quality_state": quality_state,
                        "source_selection_reason": optional("selection_reason")
                            .unwrap_or_else(|| "frozen_genomovar_definition_member".into()),
                        "historical_origin_path": optional("chromosome_original_source"),
                        "historical_origin_validation": "frozen_metadata_only_not_an_additional_packaged_source",
                        "historical_source_basis": optional("chromosome_source_basis"),
                        "assignment_status": cell(overlay, "assignment_status"),
                    }),
                });
            }
        }
        let mut ids = BTreeSet::new();
        if let Some(repeated) = references.iter().find(|r| !ids.insert(r.id.as_str())) {
            return Err(inconsistent(format!(
                "reference {} is repeated",
                repeated.id
            )));
        }
        Ok(references)
    }

    /// The pin of an archived sequence. Its length can only come from the original file, so the
    /// manifest's own record supplies it; the path and digest must be the table's.
    fn sequence(&mut self, owner: &str, path: &str, digest: &str) -> Result<Value, PackageError> {
        let recorded = self.source["references"]
            .as_array()
            .and_then(|all| all.iter().find(|r| r["reference_id"] == owner))
            .ok_or_else(|| inconsistent(format!("the source manifest has no reference {owner}")))?;
        let record = json!({
            "path": path,
            "sha256": digest,
            "bytes": recorded["source"]["bytes"],
        });
        self.pin(&record)?;
        Ok(record)
    }

    fn check_named_counts(
        &self,
        concepts: &[Concept],
        references: &[Reference],
    ) -> Result<(), PackageError> {
        let rows = self.table("named_registry").by_key();
        for concept in concepts.iter().filter(|c| c.rank != "lineage") {
            let row = rows[concept.id.as_str()];
            let represented: Vec<&Reference> = references
                .iter()
                .filter(|r| r.component == PRIMARY)
                .filter(|r| match concept.rank.as_str() {
                    "species" => r.species == concept.id,
                    _ => r.subspecies.as_deref() == Some(concept.id.as_str()),
                })
                .collect();
            let types = represented.iter().filter(|r| r.type_anchor).count();
            let owner = format!("the named registry row for {}", concept.id);
            if number(row, "primary140_reference_count", "named_registry")? != represented.len()
                || number(row, "primary140_type_count", "named_registry")? != types
            {
                return Err(inconsistent(format!(
                    "{owner} counts other primary references or types"
                )));
            }
            for (column, keep) in [
                (
                    "all_represented_historical_type_accessions",
                    (|r: &Reference| r.type_anchor) as fn(&Reference) -> bool,
                ),
                ("supplementary_reference_accessions", |r| !r.type_anchor),
                ("type_QC_exception_accessions", |r| {
                    r.quality_state == "type_QC_exception"
                }),
            ] {
                let listed = split_list(cell(row, column));
                let actual: BTreeSet<&str> = represented
                    .iter()
                    .filter(|r| keep(r))
                    .map(|r| r.accession.as_str())
                    .collect();
                if listed != actual {
                    return Err(inconsistent(format!("{owner} lists other {column}")));
                }
            }
        }
        Ok(())
    }

    fn genomovars(&self, concepts: &[Concept]) -> Result<Vec<Concept>, PackageError> {
        let by_name: BTreeMap<&str, &Concept> =
            concepts.iter().map(|c| (c.name.as_str(), c)).collect();
        let by_id: BTreeMap<&str, &Concept> = concepts.iter().map(|c| (c.id.as_str(), c)).collect();
        let definitions = &self.table("definition_references").rows;
        let crosswalk = self.table("genomovar_crosswalk").by_key();
        let mut genomovars = Vec::new();
        for group in &self.table("definition_groups").rows {
            let id = cell(group, "genomovar_id");
            let cross = crosswalk[id];
            let rows: Vec<&Row> = definitions
                .iter()
                .filter(|row| cell(row, "genomovar_id") == id)
                .collect();
            let parent = cell(cross, "parent_concept_id");
            let version = cell(cross, "version");
            let rule = cell(cross, "historical_rule");
            let parents: BTreeSet<&str> = rows
                .iter()
                .filter_map(|row| by_name.get(cell(row, "group")).map(|c| c.id.as_str()))
                .collect();
            if parents != BTreeSet::from([parent])
                || by_id.get(parent).is_none_or(|c| c.rank != "subspecies")
            {
                return Err(inconsistent(format!(
                    "genomovar {id} has an undefined or wrong parent"
                )));
            }
            if rows.iter().any(|row| {
                cell(row, "definition_version") != version || cell(row, "definition_rule") != rule
            }) {
                return Err(inconsistent(format!(
                    "genomovar {id} mixes definition versions or rules"
                )));
            }
            if number(group, "genomes", "definition_groups")? != rows.len()
                || number(cross, "definition_member_count", "genomovar_crosswalk")? != rows.len()
            {
                return Err(inconsistent(format!(
                    "genomovar {id} counts other members than it defines"
                )));
            }
            let accessions: BTreeSet<&str> =
                rows.iter().map(|row| cell(row, "accession")).collect();
            let representative = cell(group, "representative_accession");
            if cell(cross, "representative_accession") != representative
                || !accessions.contains(representative)
            {
                return Err(inconsistent(format!(
                    "the representative of genomovar {id} is not one of its members"
                )));
            }
            let concept_id = format!("ECTX-GV-{version}-{id}");
            let members: Vec<String> = accessions
                .iter()
                .map(|accession| format!("{GENOMOVAR}:{accession}"))
                .collect();
            let sampling = if rows.len() == 1 {
                "singleton_no_independent_own_group_test"
            } else {
                "multiple_definition_members"
            };
            genomovars.push(Concept {
                id: concept_id.clone(),
                rank: "genomovar".into(),
                parent: Some(parent.to_string()),
                name: id.to_string(),
                represented: true,
                value: json!({
                    "concept_id": concept_id,
                    "rank": "genomovar",
                    "parent_concept_id": parent,
                    "display_name": id,
                    "identifier_status": IDENTIFIER_STATUS,
                    "nomenclature_status": "informal_operational_ANI_group",
                    "definition_version": version,
                    "historical_definition_rule": rule,
                    "historical_definition_engine": "skani 0.3.1",
                    "definition_sequence_scope": COMPONENTS[1].2,
                    "definition_member_ids": members,
                    "representative_reference_id": format!("{GENOMOVAR}:{representative}"),
                    "representative_role": "nomination_only_not_membership_shortcut",
                    "sampling_state": sampling,
                    "reference_state": "represented_definition",
                    "validated_supported_caller_rank": false,
                    "production_membership_policy_enabled": false,
                }),
            });
        }
        Ok(genomovars)
    }

    fn historical_types(
        &self,
        concepts: &[Concept],
        references: &[Reference],
    ) -> Result<Vec<Value>, PackageError> {
        let name_of = |id: &str| {
            concepts
                .iter()
                .find(|c| c.id == id)
                .map_or("", |c| c.name.as_str())
        };
        let primary: BTreeMap<&str, &Reference> = references
            .iter()
            .filter(|r| r.component == PRIMARY)
            .map(|r| (r.accession.as_str(), r))
            .collect();
        let table = self.table("historical_types");
        let mut named = BTreeSet::new();
        for row in &table.rows {
            let taxon = cell(row, "historical_taxon_name");
            for accession in split_list(cell(row, "represented_name_bearing_accessions")) {
                let reference = primary
                    .get(accession)
                    .filter(|r| r.type_anchor)
                    .ok_or_else(|| {
                        inconsistent(format!(
                            "historical type {taxon} names {accession}, which is not a primary type anchor"
                        ))
                    })?;
                if cell(row, "current_project_species") != name_of(&reference.species)
                    || cell(row, "current_project_stratum") != name_of(&reference.stratum)
                {
                    return Err(inconsistent(format!(
                        "historical type {taxon} names other concepts than {accession} has"
                    )));
                }
                named.insert(accession);
            }
        }
        let anchors: BTreeSet<&str> = primary
            .values()
            .filter(|r| r.type_anchor)
            .map(|r| r.accession.as_str())
            .collect();
        if named != anchors {
            return Err(inconsistent(
                "the historical types table omits a primary type anchor",
            ));
        }
        Ok(table.by_key().into_values().map(|row| json!(row)).collect())
    }

    fn context(&self) -> Result<Vec<Value>, PackageError> {
        let packaged: BTreeSet<&str> = self
            .table("primary_references")
            .keys()
            .union(&self.table("definition_references").keys())
            .copied()
            .collect();
        let context = self.table("development_context").by_key();
        let mut records = Vec::new();
        for (accession, row) in context {
            if packaged.contains(accession) {
                return Err(inconsistent(format!(
                    "context record {accession} is also a packaged reference"
                )));
            }
            if cell(row, "reference_in_proposed_package") != "False" {
                return Err(inconsistent(format!(
                    "context record {accession} claims to be in the package"
                )));
            }
            let mut kept = row.clone();
            kept.remove("whole_fasta");
            kept.remove("whole_sha256");
            records.push(json!(kept));
        }
        Ok(records)
    }

    /// The cohort is its own record set. Each row's membership flags and evidence flags must
    /// follow from the package; its coverage category is a scientific record, summarised as is.
    fn cohort(&self, concepts: &[Concept]) -> Result<BTreeMap<String, u64>, PackageError> {
        let primary = self.table("primary_references").keys();
        let definitions = self.table("definition_references").keys();
        let mut summary: BTreeMap<String, u64> = BTreeMap::new();
        for row in &self.table("cohort_coverage").rows {
            let accession = cell(row, "accession");
            let owner = format!("cohort row {accession}");
            if cell(row, "validated_automatic_assignment") != "False" {
                return Err(inconsistent(format!("{owner} implies a validated caller")));
            }
            if cell(row, "in_primary140") != python_bool(primary.contains(accession))
                || cell(row, "in_genomovar173") != python_bool(definitions.contains(accession))
            {
                return Err(inconsistent(format!(
                    "{owner} gives another component membership than the package"
                )));
            }
            for rank in ["species", "subspecies"] {
                let id = cell(row, &format!("{rank}_concept_id"));
                let concept = concepts.iter().find(|c| c.id == id);
                if !id.is_empty() && concept.is_none_or(|c| c.rank != rank) {
                    return Err(inconsistent(format!(
                        "{owner} names {id}, which is not a {rank} concept"
                    )));
                }
                let present = concept.is_some_and(|c| c.represented);
                if cell(row, &format!("{rank}_reference_evidence_present")) != python_bool(present)
                {
                    return Err(inconsistent(format!(
                        "{owner} records {rank} reference evidence the package does not hold"
                    )));
                }
            }
            // The research builder leaves this column unchecked; every frozen value is a lineage.
            let lineage = cell(row, "lineage_context_id");
            if !lineage.is_empty()
                && !concepts
                    .iter()
                    .any(|c| c.id == lineage && c.rank == "lineage")
            {
                return Err(inconsistent(format!(
                    "{owner} names {lineage}, which is not a lineage concept"
                )));
            }
            *summary
                .entry(cell(row, "coverage_category").to_string())
                .or_default() += 1;
        }
        Ok(summary)
    }

    fn components(&mut self, references: &[Reference]) -> Result<Vec<Value>, PackageError> {
        let configured = object(&self.recipe["components"], "the recipe components")?;
        let names: BTreeSet<&str> = COMPONENTS.iter().map(|c| c.0).collect();
        if configured
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != names
        {
            return Err(inconsistent("the recipe names other components"));
        }
        let mut components = Vec::new();
        for (id, role, scope) in COMPONENTS {
            let configuration = object(&configured[id], "a recipe component")?;
            expect_keys(
                configuration,
                &[
                    "build_records",
                    "database_files",
                    "skani_binary",
                    "source_list",
                ],
                &format!("recipe component {id}"),
            )?;
            let members: Vec<&Reference> =
                references.iter().filter(|r| r.component == id).collect();
            let labels: Vec<&str> = members.iter().map(|r| r.label.as_str()).collect();
            let declared: BTreeSet<&str> = labels.iter().copied().collect();
            if declared.len() != labels.len() {
                return Err(inconsistent(format!(
                    "component {id} gives two references one search label"
                )));
            }

            let list = &configuration["source_list"];
            self.pin(list)?;
            let text = std::str::from_utf8(self.copied(list)?)
                .map_err(|_| inconsistent(format!("the source list of {id} is not UTF-8")))?;
            let lines: Vec<&str> = text.lines().collect();
            let listed: BTreeSet<&str> = lines.iter().copied().collect();
            if text.contains('\r') || lines.len() != listed.len() || listed != declared {
                return Err(inconsistent(format!(
                    "the source list of component {id} does not list its sequences"
                )));
            }

            let files = array(&configuration["database_files"], "database files")?;
            let mut by_name: BTreeMap<&str, &Value> = BTreeMap::new();
            let mut directories = BTreeSet::new();
            for file in files {
                self.pin(file)?;
                let path = file_record(file)?;
                let (directory, name) = path.rsplit_once('/').unwrap_or(("", path));
                directories.insert(directory);
                if by_name.insert(name, file).is_some() {
                    return Err(inconsistent(format!("component {id} repeats {name}")));
                }
            }
            if by_name.keys().copied().ne(DATABASE_FILES) || directories.len() != 1 {
                return Err(inconsistent(format!(
                    "component {id} does not have exactly one database directory of {}",
                    DATABASE_FILES.join(", ")
                )));
            }
            let sketches = by_name["sketches.db"]["bytes"].as_u64().unwrap_or(0);
            let entries = index::parse(self.copied(by_name["index.db"])?, sketches)?;
            index::check_membership(&entries, &declared, id)?;
            let builds = array(&configuration["build_records"], "build records")?;
            if builds.is_empty() {
                return Err(inconsistent(format!("component {id} has no build records")));
            }
            for record in builds {
                self.pin(record)?;
            }
            self.pin(&configuration["skani_binary"])?;

            let mut database_files: Vec<&Value> = files.iter().collect();
            database_files.sort_by_key(|file| file["path"].as_str().unwrap_or_default());
            let mut index_entries: Vec<&Entry> = entries.iter().collect();
            index_entries.sort_by(|a, b| a.label.cmp(&b.label));
            let mut ids: Vec<&str> = members.iter().map(|r| r.id.as_str()).collect();
            ids.sort_unstable();
            components.push(json!({
                "component_id": id,
                "reference_role": role,
                "sequence_scope": scope,
                "sequence_scope_basis": "frozen_source_manifest_not_new_replicon_detection",
                "reference_ids": ids,
                "engine": ENGINE,
                "engine_version": ENGINE_VERSION,
                "database_format": DATABASE_FORMAT,
                "database_files": database_files,
                "database_index_entries": index_entries
                    .iter()
                    .map(|e| json!({"path": e.label, "offset": e.offset, "bytes": e.bytes}))
                    .collect::<Vec<_>>(),
                "search_label_semantics": "exact_archived_index_label_not_a_deployed_FASTA_requirement",
                "provenance_state": "archived_build_records_hash_checked_not_reexecuted",
                "sketch_payload_recomputed": false,
            }));
        }
        components.sort_by(|a, b| a["component_id"].as_str().cmp(&b["component_id"].as_str()));
        Ok(components)
    }

    /// The bytes of the packaged copy of an archived file.
    fn copied(&self, record: &Value) -> Result<&'a [u8], PackageError> {
        let path = file_record(record)?;
        let asset = self
            .assets
            .iter()
            .find(|asset| asset.source_path == path)
            .ok_or_else(|| inconsistent(format!("the manifest lists no copy of {path}")))?;
        self.held
            .get(asset.path.as_str())
            .map(Vec::as_slice)
            .ok_or_else(|| inconsistent(format!("{} was not read", asset.path)))
    }

    fn counts(&self, references: &[Reference]) -> Result<Value, PackageError> {
        let rows = |name: &str| self.table(name).rows.len();
        let accessions: BTreeSet<&str> = references.iter().map(|r| r.accession.as_str()).collect();
        let cohort = &self.table("cohort_coverage").rows;
        Ok(json!({
            "primary_whole_references": rows("primary_references"),
            "historical_chromosome_definition_references": rows("definition_references"),
            "unique_accessions": accessions.len(),
            "reference_scope_records": references.len(),
            "named_concepts": rows("named_registry"),
            "context_lineage_concepts": rows("lineage_registry"),
            "genomovar_concepts": rows("definition_groups"),
            "development_context_records_not_packaged": rows("development_context"),
            "eligible_cohort_records": cohort.len(),
            "eligible_species_reference_represented": cohort
                .iter()
                .filter(|row| cell(row, "species_reference_evidence_present") == "True")
                .count(),
            "primary_historical_type_accessions": references
                .iter()
                .filter(|r| r.component == PRIMARY && r.type_anchor)
                .count(),
        }))
    }
}

/// The path of a pinned file record, which must hold exactly an absolute path, a lowercase
/// SHA-256 and a byte length.
pub(super) fn file_record(record: &Value) -> Result<&str, PackageError> {
    let fields = record
        .as_object()
        .filter(|fields| fields.len() == 3)
        .ok_or_else(|| inconsistent("a file record must hold exactly path, sha256 and bytes"))?;
    let path = fields
        .get("path")
        .and_then(Value::as_str)
        .filter(|path| path.starts_with('/') && !path.contains('\0'))
        .ok_or_else(|| inconsistent("a file record has no absolute path"))?;
    if !fields
        .get("sha256")
        .and_then(Value::as_str)
        .is_some_and(is_sha256)
        || fields.get("bytes").and_then(Value::as_u64).is_none()
    {
        return Err(inconsistent(format!(
            "the file record for {path} has no valid digest or length"
        )));
    }
    Ok(path)
}

fn expect_keys(fields: &Map<String, Value>, keys: &[&str], name: &str) -> Result<(), PackageError> {
    if fields.keys().map(String::as_str).ne(keys.iter().copied()) {
        return Err(inconsistent(format!(
            "{name} has missing or unknown fields"
        )));
    }
    Ok(())
}

fn identifier<'v>(value: &'v Value, name: &str) -> Result<&'v str, PackageError> {
    value
        .as_str()
        .filter(|text| {
            !text.is_empty()
                && text
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        })
        .ok_or_else(|| inconsistent(format!("the recipe has an invalid {name}")))
}

fn object<'v>(value: &'v Value, name: &str) -> Result<&'v Map<String, Value>, PackageError> {
    value
        .as_object()
        .ok_or_else(|| inconsistent(format!("{name} is not an object")))
}

fn array<'v>(value: &'v Value, name: &str) -> Result<&'v Vec<Value>, PackageError> {
    value
        .as_array()
        .ok_or_else(|| inconsistent(format!("{name} is not a list")))
}

fn number(row: &Row, column: &str, table: &str) -> Result<usize, PackageError> {
    let text = cell(row, column);
    text.parse()
        .ok()
        .filter(|_| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()))
        .ok_or_else(|| inconsistent(format!("{table} has a non-numeric {column}: {text}")))
}

fn split_list(text: &str) -> BTreeSet<&str> {
    text.split(';').filter(|item| !item.is_empty()).collect()
}

fn python_bool(value: bool) -> &'static str {
    if value {
        "True"
    } else {
        "False"
    }
}

/// The first place where `actual` differs from `expected`, as a readable path.
fn first_difference(expected: &Value, actual: &Value, place: &str) -> Option<String> {
    match (expected, actual) {
        (Value::Object(want), Value::Object(have)) => {
            let keys: BTreeSet<&String> = want.keys().chain(have.keys()).collect();
            keys.into_iter()
                .find_map(|key| match (want.get(key), have.get(key)) {
                    (Some(w), Some(h)) => first_difference(w, h, &format!("{place}.{key}")),
                    _ => Some(format!("{place}.{key}")),
                })
        }
        (Value::Array(want), Value::Array(have)) => {
            if want.len() != have.len() {
                return Some(format!("the length of {place}"));
            }
            want.iter().zip(have).enumerate().find_map(|(i, (w, h))| {
                let label = h
                    .get("reference_id")
                    .or_else(|| h.get("concept_id"))
                    .or_else(|| h.get("component_id"))
                    .and_then(Value::as_str)
                    .map_or_else(|| i.to_string(), str::to_string);
                first_difference(w, h, &format!("{place}[{label}]"))
            })
        }
        _ if expected == actual => None,
        _ => Some(place.to_string()),
    }
}
