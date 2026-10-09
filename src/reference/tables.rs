//! The packaged tables restate the source manifest in tabular form. Every row must agree with the
//! source manifest, and the counts the rows give must agree with the counts recorded there and in
//! the export attestation.

use std::collections::{BTreeMap, BTreeSet};

use super::source::{Concept, Counts, Export, SourceManifest, SourceReference};
use super::{inconsistent, role_asset, Asset, PackageError};

type Row = BTreeMap<String, String>;

const PRIMARY: &str = "primary_whole";
const GENOMOVAR: &str = "genomovar_chromosome";

const TABLES: [(&str, &[&str]); 11] = [
    (
        "assignments",
        &[
            "accession",
            "project_species",
            "project_subspecies",
            "assignment_status",
            "usage_restrictions",
            "genomovar_id",
            "genomovar_version",
            "genomovar_rule",
        ],
    ),
    (
        "cohort_coverage",
        &[
            "accession",
            "species_concept_id",
            "subspecies_concept_id",
            "coverage_category",
            "species_reference_evidence_present",
            "subspecies_reference_evidence_present",
            "in_primary140",
            "in_genomovar173",
            "validated_automatic_assignment",
        ],
    ),
    (
        "definition_groups",
        &["genomovar_id", "genomes", "representative_accession"],
    ),
    (
        "definition_memberships",
        &[
            "accession",
            "project_species",
            "project_subspecies_proposal",
            "genomovar_id",
            "genomovar_version",
            "genomovar_rule",
        ],
    ),
    (
        "definition_references",
        &[
            "accession",
            "genomovar_id",
            "definition_version",
            "definition_rule",
            "group",
            "partition",
            "chromosome_fasta",
            "chromosome_sha256",
            "chromosome_original_source",
            "chromosome_source_basis",
            "is_type",
            "qc_pass",
            "quality_flags",
        ],
    ),
    (
        "development_context",
        &[
            "accession",
            "reference_in_proposed_package",
            "context_disposition",
            "whole_fasta",
            "whole_sha256",
        ],
    ),
    (
        "genomovar_crosswalk",
        &[
            "genomovar_id",
            "parent_concept_id",
            "status",
            "version",
            "historical_rule",
            "definition_member_count",
            "representative_accession",
        ],
    ),
    (
        "historical_types",
        &[
            "historical_taxon_name",
            "represented_name_bearing_accessions",
            "current_project_species",
            "current_project_stratum",
        ],
    ),
    (
        "lineage_registry",
        &[
            "concept_id",
            "rank",
            "project_label",
            "nomenclature_status",
            "primary140_reference_count",
            "genomovar173_reference_count",
            "represented_reference_evidence",
            "validated_supported_caller_rank",
        ],
    ),
    (
        "named_registry",
        &[
            "concept_id",
            "rank",
            "parent_concept_id",
            "project_name",
            "nomenclature_status",
            "primary140_reference_count",
            "primary140_type_count",
            "all_represented_historical_type_accessions",
            "supplementary_reference_accessions",
            "type_QC_exception_accessions",
            "represented_reference_evidence",
            "validated_supported_caller_rank",
        ],
    ),
    (
        "primary_references",
        &[
            "accession",
            "group",
            "project_species",
            "project_subspecies",
            "named_target",
            "type_anchor",
            "partition",
            "qc_pass",
            "quality_flags",
            "whole_fasta",
            "whole_sha256",
            "selection_reason",
        ],
    ),
];

pub(super) struct Tables(BTreeMap<&'static str, Vec<Row>>);

impl Tables {
    pub(super) fn load(
        assets: &[Asset],
        held: &BTreeMap<&str, Vec<u8>>,
    ) -> Result<Self, PackageError> {
        let mut tables = BTreeMap::new();
        for (name, columns) in TABLES {
            let asset = role_asset(assets, &format!("table:{name}"))?;
            let bytes = held
                .get(asset.path.as_str())
                .ok_or_else(|| inconsistent(format!("{} was not read", asset.path)))?;
            tables.insert(name, parse_table(name, bytes, columns)?);
        }
        Ok(Self(tables))
    }

    pub(super) fn check(
        &self,
        source: &SourceManifest,
        export: &Export,
    ) -> Result<(), PackageError> {
        self.check_counts(source, export)?;
        check_genomovars(source)?;
        self.check_named(source)?;
        self.check_lineage(source)?;
        self.check_cohort(source)?;
        self.check_primary(source)?;
        self.check_definition_members(source)?;
        self.check_definition_groups(source)?;
        self.check_context(source)?;
        self.check_historical(source)?;
        self.check_assignments(source)
    }

    fn rows(&self, name: &str) -> &[Row] {
        &self.0[name]
    }

    fn check_counts(&self, source: &SourceManifest, export: &Export) -> Result<(), PackageError> {
        let cohort = self.rows("cohort_coverage");
        let primary = self.rows("primary_references");
        let concepts = |rank: &str| {
            source
                .concepts
                .iter()
                .filter(|concept| concept.rank == rank)
                .count() as u64
        };
        let accessions: BTreeSet<&str> = source
            .references
            .iter()
            .map(|reference| reference.accession.as_str())
            .collect();
        let derived = Counts {
            context_lineage_concepts: concepts("lineage"),
            development_context_records_not_packaged: self.rows("development_context").len() as u64,
            eligible_cohort_records: cohort.len() as u64,
            eligible_species_reference_represented: cohort
                .iter()
                .filter(|row| cell(row, "species_reference_evidence_present") == "True")
                .count() as u64,
            genomovar_concepts: concepts("genomovar"),
            historical_chromosome_definition_references: self.rows("definition_references").len()
                as u64,
            named_concepts: self.rows("named_registry").len() as u64,
            primary_historical_type_accessions: primary
                .iter()
                .filter(|row| cell(row, "type_anchor") == "1")
                .count() as u64,
            primary_whole_references: primary.len() as u64,
            reference_scope_records: source.references.len() as u64,
            unique_accessions: accessions.len() as u64,
        };
        if derived != source.counts || derived != export.local_validation.counts {
            return Err(inconsistent(
                "the table rows do not give the counts the source manifest and export record",
            ));
        }
        Ok(())
    }

    fn check_named(&self, source: &SourceManifest) -> Result<(), PackageError> {
        let concepts: Vec<&Concept> = source
            .concepts
            .iter()
            .filter(|concept| matches!(concept.rank.as_str(), "species" | "subspecies"))
            .collect();
        let rows = self.rows("named_registry");
        check_registry(rows, &concepts, "named_registry", "project_name", true)?;
        let by_id = by_key(rows, "concept_id")?;
        for concept in concepts {
            let row = by_id[concept.concept_id.as_str()];
            let covered: Vec<&SourceReference> = source
                .references
                .iter()
                .filter(|reference| reference.component_id == PRIMARY && covers(reference, concept))
                .collect();
            let types = covered
                .iter()
                .filter(|reference| reference.type_anchor)
                .count();
            expect_row(
                row,
                &format!("named registry {}", concept.concept_id),
                &[
                    ("primary140_reference_count", &covered.len().to_string()),
                    ("primary140_type_count", &types.to_string()),
                ],
            )?;
        }
        Ok(())
    }

    fn check_lineage(&self, source: &SourceManifest) -> Result<(), PackageError> {
        let concepts: Vec<&Concept> = source
            .concepts
            .iter()
            .filter(|concept| concept.rank == "lineage")
            .collect();
        check_registry(
            self.rows("lineage_registry"),
            &concepts,
            "lineage_registry",
            "project_label",
            false,
        )
    }

    fn check_cohort(&self, source: &SourceManifest) -> Result<(), PackageError> {
        let by_accession = by_key(self.rows("cohort_coverage"), "accession")?;
        let expected: BTreeSet<&str> = source
            .references
            .iter()
            .map(|reference| reference.accession.as_str())
            .chain(
                source
                    .context_not_packaged
                    .iter()
                    .map(|context| context.accession.as_str()),
            )
            .collect();
        if keys(&by_accession) != expected {
            return Err(inconsistent(
                "the cohort table does not name the cohort accessions",
            ));
        }

        let mut summary: BTreeMap<String, u64> = BTreeMap::new();
        for reference in &source.references {
            let category = match reference.component_id.as_str() {
                PRIMARY => "primary_reference",
                GENOMOVAR => "genomovar_definition_member",
                _ => return Err(inconsistent("a reference names no known component")),
            };
            let species = reference.species_id.as_str();
            let subspecies = reference.subspecies_id.as_deref().unwrap_or("");
            expect_row(
                by_accession[reference.accession.as_str()],
                &format!("cohort {}", reference.accession),
                &[
                    ("species_concept_id", species),
                    ("subspecies_concept_id", subspecies),
                    ("coverage_category", category),
                    (
                        "species_reference_evidence_present",
                        flag(!species.is_empty()),
                    ),
                    (
                        "subspecies_reference_evidence_present",
                        flag(!subspecies.is_empty()),
                    ),
                    ("in_primary140", flag(category == "primary_reference")),
                    (
                        "in_genomovar173",
                        flag(category == "genomovar_definition_member"),
                    ),
                    ("validated_automatic_assignment", "False"),
                ],
            )?;
            *summary.entry(category.to_string()).or_default() += 1;
        }
        for context in &source.context_not_packaged {
            expect_row(
                by_accession[context.accession.as_str()],
                &format!("cohort {}", context.accession),
                &[
                    ("species_concept_id", ""),
                    ("subspecies_concept_id", ""),
                    ("coverage_category", "context_only"),
                    ("species_reference_evidence_present", "False"),
                    ("subspecies_reference_evidence_present", "False"),
                    ("in_primary140", "False"),
                    ("in_genomovar173", "False"),
                    ("validated_automatic_assignment", "False"),
                ],
            )?;
            *summary.entry("context_only".to_string()).or_default() += 1;
        }
        if summary != source.cohort_coverage_summary {
            return Err(inconsistent(
                "the cohort coverage categories differ from the source manifest summary",
            ));
        }
        Ok(())
    }

    fn check_primary(&self, source: &SourceManifest) -> Result<(), PackageError> {
        let references: Vec<&SourceReference> = source
            .references
            .iter()
            .filter(|reference| reference.component_id == PRIMARY)
            .collect();
        let by_accession = by_key(self.rows("primary_references"), "accession")?;
        let expected: BTreeSet<&str> = references
            .iter()
            .map(|reference| reference.accession.as_str())
            .collect();
        if keys(&by_accession) != expected {
            return Err(inconsistent(
                "the primary table does not name the primary references",
            ));
        }
        for reference in references {
            let species = name_of(source, &reference.species_id)?;
            let subspecies = subspecies_name(source, reference)?;
            expect_row(
                by_accession[reference.accession.as_str()],
                &format!("primary {}", reference.accession),
                &[
                    ("group", subspecies.unwrap_or(species)),
                    ("project_species", species),
                    ("project_subspecies", subspecies.map_or("", epithet)),
                    ("type_anchor", bit(reference.type_anchor)),
                    ("quality_flags", reference.quality_flags.as_str()),
                    ("whole_fasta", reference.source.path.as_str()),
                    ("whole_sha256", reference.source.sha256.as_str()),
                ],
            )?;
        }
        Ok(())
    }

    fn check_definition_members(&self, source: &SourceManifest) -> Result<(), PackageError> {
        let members: Vec<&SourceReference> = source
            .references
            .iter()
            .filter(|reference| reference.component_id == GENOMOVAR)
            .collect();
        let expected: BTreeSet<&str> = members
            .iter()
            .map(|reference| reference.accession.as_str())
            .collect();
        let definitions = by_key(self.rows("definition_references"), "accession")?;
        let memberships = by_key(self.rows("definition_memberships"), "accession")?;
        if keys(&definitions) != expected || keys(&memberships) != expected {
            return Err(inconsistent(
                "the definition tables do not name the genomovar definition members",
            ));
        }
        for reference in members {
            let genomovar = genomovar_of(source, reference).ok_or_else(|| {
                inconsistent(format!(
                    "{} belongs to no genomovar concept",
                    reference.reference_id
                ))
            })?;
            let species = name_of(source, &reference.species_id)?;
            let subspecies = subspecies_name(source, reference)?.unwrap_or("");
            expect_row(
                definitions[reference.accession.as_str()],
                &format!("definition reference {}", reference.accession),
                &[
                    ("genomovar_id", genomovar.display_name.as_str()),
                    ("definition_version", opt(&genomovar.definition_version)),
                    (
                        "definition_rule",
                        opt(&genomovar.historical_definition_rule),
                    ),
                    ("group", group_name(source, reference)?),
                    ("chromosome_fasta", reference.source.path.as_str()),
                    ("chromosome_sha256", reference.source.sha256.as_str()),
                    ("is_type", bit(reference.type_anchor)),
                    ("quality_flags", reference.quality_flags.as_str()),
                ],
            )?;
            expect_row(
                memberships[reference.accession.as_str()],
                &format!("definition membership {}", reference.accession),
                &[
                    ("project_species", species),
                    ("project_subspecies_proposal", subspecies),
                    ("genomovar_id", genomovar.display_name.as_str()),
                    ("genomovar_version", opt(&genomovar.definition_version)),
                    ("genomovar_rule", opt(&genomovar.historical_definition_rule)),
                ],
            )?;
        }
        Ok(())
    }

    fn check_definition_groups(&self, source: &SourceManifest) -> Result<(), PackageError> {
        let genomovars: Vec<&Concept> = source
            .concepts
            .iter()
            .filter(|concept| concept.rank == "genomovar")
            .collect();
        let names: BTreeSet<&str> = genomovars
            .iter()
            .map(|genomovar| genomovar.display_name.as_str())
            .collect();
        let groups = by_key(self.rows("definition_groups"), "genomovar_id")?;
        let crosswalk = by_key(self.rows("genomovar_crosswalk"), "genomovar_id")?;
        if keys(&groups) != names || keys(&crosswalk) != names {
            return Err(inconsistent(
                "the definition tables do not name the genomovar concepts",
            ));
        }
        for genomovar in genomovars {
            let members = genomovar.definition_member_ids.len().to_string();
            let representative = representative_accession(source, genomovar)?;
            let parent = genomovar.parent_concept_id.as_deref().unwrap_or("");
            expect_row(
                groups[genomovar.display_name.as_str()],
                &format!("definition group {}", genomovar.display_name),
                &[
                    ("genomes", members.as_str()),
                    ("representative_accession", representative),
                ],
            )?;
            expect_row(
                crosswalk[genomovar.display_name.as_str()],
                &format!("genomovar crosswalk {}", genomovar.display_name),
                &[
                    ("parent_concept_id", parent),
                    ("status", genomovar.nomenclature_status.as_str()),
                    ("version", opt(&genomovar.definition_version)),
                    (
                        "historical_rule",
                        opt(&genomovar.historical_definition_rule),
                    ),
                    ("definition_member_count", members.as_str()),
                    ("representative_accession", representative),
                ],
            )?;
        }
        Ok(())
    }

    fn check_context(&self, source: &SourceManifest) -> Result<(), PackageError> {
        let packaged: BTreeSet<&str> = source
            .references
            .iter()
            .map(|reference| reference.accession.as_str())
            .collect();
        let context: BTreeSet<&str> = source
            .context_not_packaged
            .iter()
            .map(|record| record.accession.as_str())
            .collect();
        if !packaged.is_disjoint(&context)
            || source
                .context_not_packaged
                .iter()
                .any(|record| record.reference_in_proposed_package != "False")
        {
            return Err(inconsistent(
                "a context record is also a packaged reference",
            ));
        }
        let rows = by_key(self.rows("development_context"), "accession")?;
        if keys(&rows) != context {
            return Err(inconsistent(
                "the development context table does not name the context records",
            ));
        }
        for record in &source.context_not_packaged {
            expect_row(
                rows[record.accession.as_str()],
                &format!("development context {}", record.accession),
                &[
                    (
                        "reference_in_proposed_package",
                        record.reference_in_proposed_package.as_str(),
                    ),
                    ("context_disposition", record.context_disposition.as_str()),
                ],
            )?;
        }
        Ok(())
    }

    fn check_historical(&self, source: &SourceManifest) -> Result<(), PackageError> {
        let rows = by_key(self.rows("historical_types"), "historical_taxon_name")?;
        let names: BTreeSet<&str> = source
            .historical_type_crosswalk
            .iter()
            .map(|entry| entry.historical_taxon_name.as_str())
            .collect();
        if keys(&rows) != names {
            return Err(inconsistent(
                "the historical types table does not name the source entries",
            ));
        }
        for entry in &source.historical_type_crosswalk {
            let represented = source
                .references
                .iter()
                .find(|reference| reference.accession == entry.represented_name_bearing_accessions)
                .ok_or_else(|| {
                    inconsistent(format!(
                        "{} names no packaged reference",
                        entry.represented_name_bearing_accessions
                    ))
                })?;
            if name_of(source, &represented.species_id)? != entry.current_project_species {
                return Err(inconsistent(format!(
                    "{} is not a reference of {}",
                    entry.represented_name_bearing_accessions, entry.current_project_species
                )));
            }
            expect_row(
                rows[entry.historical_taxon_name.as_str()],
                &format!("historical type {}", entry.historical_taxon_name),
                &[
                    (
                        "represented_name_bearing_accessions",
                        entry.represented_name_bearing_accessions.as_str(),
                    ),
                    (
                        "current_project_species",
                        entry.current_project_species.as_str(),
                    ),
                    (
                        "current_project_stratum",
                        entry.current_project_stratum.as_str(),
                    ),
                ],
            )?;
        }
        Ok(())
    }

    fn check_assignments(&self, source: &SourceManifest) -> Result<(), PackageError> {
        let rows = by_key(self.rows("assignments"), "accession")?;
        let expected: BTreeSet<&str> = source
            .references
            .iter()
            .map(|reference| reference.accession.as_str())
            .chain(
                source
                    .context_not_packaged
                    .iter()
                    .map(|context| context.accession.as_str()),
            )
            .collect();
        if keys(&rows) != expected {
            return Err(inconsistent(
                "the assignments table does not name the cohort accessions",
            ));
        }
        for reference in &source.references {
            let species = name_of(source, &reference.species_id)?;
            let subspecies = subspecies_name(source, reference)?.map_or("", epithet);
            let (genomovar_id, version, rule) = match genomovar_of(source, reference) {
                Some(genomovar) => (
                    genomovar.display_name.as_str(),
                    opt(&genomovar.definition_version),
                    opt(&genomovar.historical_definition_rule),
                ),
                None => ("", "", ""),
            };
            expect_row(
                rows[reference.accession.as_str()],
                &format!("assignment {}", reference.accession),
                &[
                    ("project_species", species),
                    ("project_subspecies", subspecies),
                    ("usage_restrictions", ""),
                    ("genomovar_id", genomovar_id),
                    ("genomovar_version", version),
                    ("genomovar_rule", rule),
                ],
            )?;
        }
        for context in &source.context_not_packaged {
            expect_row(
                rows[context.accession.as_str()],
                &format!("assignment {}", context.accession),
                &[
                    ("usage_restrictions", "context_only"),
                    ("genomovar_id", ""),
                    ("genomovar_version", ""),
                    ("genomovar_rule", ""),
                ],
            )?;
        }
        Ok(())
    }
}

fn check_genomovars(source: &SourceManifest) -> Result<(), PackageError> {
    let expected: BTreeSet<&str> = source
        .references
        .iter()
        .filter(|reference| reference.component_id == GENOMOVAR)
        .map(|reference| reference.reference_id.as_str())
        .collect();
    let mut covered: BTreeSet<&str> = BTreeSet::new();
    let mut listed = 0;
    for genomovar in source
        .concepts
        .iter()
        .filter(|concept| concept.rank == "genomovar")
    {
        let subspecies = genomovar
            .parent_concept_id
            .as_deref()
            .and_then(|id| source.concept(id))
            .filter(|concept| concept.rank == "subspecies")
            .ok_or_else(|| {
                inconsistent(format!(
                    "genomovar {} has no subspecies parent",
                    genomovar.display_name
                ))
            })?;
        if genomovar.definition_version.is_none() || genomovar.historical_definition_rule.is_none()
        {
            return Err(inconsistent(format!(
                "genomovar {} has no definition version or rule",
                genomovar.display_name
            )));
        }
        let members: BTreeSet<&str> = genomovar
            .definition_member_ids
            .iter()
            .map(String::as_str)
            .collect();
        if !members.is_subset(&expected)
            || genomovar
                .representative_reference_id
                .as_deref()
                .is_none_or(|id| !members.contains(id))
        {
            return Err(inconsistent(format!(
                "genomovar {} names members or a representative outside its definition",
                genomovar.display_name
            )));
        }
        for reference in source
            .references
            .iter()
            .filter(|reference| members.contains(reference.reference_id.as_str()))
        {
            if reference.subspecies_id.as_deref() != Some(subspecies.concept_id.as_str()) {
                return Err(inconsistent(format!(
                    "{} is not a member of the subspecies of {}",
                    reference.reference_id, genomovar.display_name
                )));
            }
        }
        listed += members.len();
        covered.extend(members);
    }
    if covered != expected || listed != expected.len() {
        return Err(inconsistent(
            "the genomovar concepts do not partition the definition references",
        ));
    }
    Ok(())
}

fn check_registry(
    rows: &[Row],
    concepts: &[&Concept],
    table: &str,
    name_column: &str,
    has_parent: bool,
) -> Result<(), PackageError> {
    let by_id = by_key(rows, "concept_id")?;
    let ids: BTreeSet<&str> = concepts
        .iter()
        .map(|concept| concept.concept_id.as_str())
        .collect();
    if keys(&by_id) != ids {
        return Err(inconsistent(format!(
            "{table} does not name the concepts it should"
        )));
    }
    for concept in concepts {
        let row = by_id[concept.concept_id.as_str()];
        let annotation = concept.source_annotation.as_ref().ok_or_else(|| {
            inconsistent(format!("{} has no source annotation", concept.concept_id))
        })?;
        if !annotation.keys().eq(row.keys())
            || row
                .iter()
                .any(|(column, value)| annotation.get(column) != Some(value))
        {
            return Err(inconsistent(format!(
                "the {table} row for {} differs from its source annotation",
                concept.concept_id
            )));
        }
        if cell(row, "rank") != concept.rank.as_str()
            || cell(row, "nomenclature_status") != concept.nomenclature_status.as_str()
            || cell(row, name_column) != concept.display_name.as_str()
            || (has_parent && cell(row, "parent_concept_id") != opt(&concept.parent_concept_id))
        {
            return Err(inconsistent(format!(
                "the {table} row for {} differs from the concept",
                concept.concept_id
            )));
        }
    }
    Ok(())
}

fn parse_table(name: &str, bytes: &[u8], columns: &[&str]) -> Result<Vec<Row>, PackageError> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| inconsistent(format!("{name} is not UTF-8")))?;
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or_else(|| inconsistent(format!("{name} has no header")))?;
    if header.split('\t').ne(columns.iter().copied()) {
        return Err(inconsistent(format!(
            "{name} has another header than the package names"
        )));
    }
    lines
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() != columns.len() {
                return Err(inconsistent(format!(
                    "{name} has a row of {} fields, not {}",
                    fields.len(),
                    columns.len()
                )));
            }
            Ok(columns
                .iter()
                .zip(fields)
                .map(|(column, field)| (column.to_string(), field.to_string()))
                .collect())
        })
        .collect()
}

fn by_key<'a>(rows: &'a [Row], column: &str) -> Result<BTreeMap<&'a str, &'a Row>, PackageError> {
    let mut keyed = BTreeMap::new();
    for row in rows {
        if keyed.insert(cell(row, column), row).is_some() {
            return Err(inconsistent(format!(
                "a table repeats {}",
                cell(row, column)
            )));
        }
    }
    Ok(keyed)
}

fn keys<'a>(rows: &BTreeMap<&'a str, &Row>) -> BTreeSet<&'a str> {
    rows.keys().copied().collect()
}

fn expect_row(row: &Row, owner: &str, pairs: &[(&str, &str)]) -> Result<(), PackageError> {
    if pairs
        .iter()
        .all(|(column, value)| cell(row, column) == *value)
    {
        Ok(())
    } else {
        Err(inconsistent(format!(
            "the {owner} row differs from the source manifest"
        )))
    }
}

fn cell<'a>(row: &'a Row, column: &str) -> &'a str {
    row.get(column).map_or("", String::as_str)
}

fn opt(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("")
}

fn flag(value: bool) -> &'static str {
    if value {
        "True"
    } else {
        "False"
    }
}

fn bit(value: bool) -> &'static str {
    if value {
        "1"
    } else {
        "0"
    }
}

fn epithet(name: &str) -> &str {
    name.rsplit(' ').next().unwrap_or(name)
}

fn name_of<'a>(source: &'a SourceManifest, id: &str) -> Result<&'a str, PackageError> {
    source
        .concept(id)
        .map(|concept| concept.display_name.as_str())
        .ok_or_else(|| inconsistent(format!("{id} is not a concept")))
}

fn subspecies_name<'a>(
    source: &'a SourceManifest,
    reference: &SourceReference,
) -> Result<Option<&'a str>, PackageError> {
    reference
        .subspecies_id
        .as_deref()
        .map(|id| name_of(source, id))
        .transpose()
}

fn group_name<'a>(
    source: &'a SourceManifest,
    reference: &SourceReference,
) -> Result<&'a str, PackageError> {
    match subspecies_name(source, reference)? {
        Some(name) => Ok(name),
        None => name_of(source, &reference.species_id),
    }
}

fn covers(reference: &SourceReference, concept: &Concept) -> bool {
    match concept.rank.as_str() {
        "species" => reference.species_id == concept.concept_id,
        _ => reference.subspecies_id.as_deref() == Some(concept.concept_id.as_str()),
    }
}

fn genomovar_of<'a>(
    source: &'a SourceManifest,
    reference: &SourceReference,
) -> Option<&'a Concept> {
    source.concepts.iter().find(|concept| {
        concept.rank == "genomovar"
            && concept
                .definition_member_ids
                .contains(&reference.reference_id)
    })
}

fn representative_accession<'a>(
    source: &'a SourceManifest,
    genomovar: &Concept,
) -> Result<&'a str, PackageError> {
    let id = opt(&genomovar.representative_reference_id);
    source
        .references
        .iter()
        .find(|reference| reference.reference_id == id)
        .map(|reference| reference.accession.as_str())
        .ok_or_else(|| {
            inconsistent(format!(
                "genomovar {} has no representative",
                genomovar.display_name
            ))
        })
}
