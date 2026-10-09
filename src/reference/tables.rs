//! The packaged registry and reference tables. Each is a tab-separated table with a header; a
//! table may carry more columns than the verifier reads, but never fewer, and no row may be
//! ragged. Rows are keyed by one column whose values must be present, unpadded and unique.

use std::collections::{BTreeMap, BTreeSet};

use super::{inconsistent, role_asset, Asset, PackageError};

pub(super) type Row = BTreeMap<String, String>;

/// Each table, the column that keys it, and the columns the verifier reads from it.
pub(super) const TABLES: [(&str, &str, &[&str]); 11] = [
    (
        "assignments",
        "accession",
        &[
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
        "accession",
        &[
            "coverage_category",
            "species_concept_id",
            "subspecies_concept_id",
            "species_reference_evidence_present",
            "subspecies_reference_evidence_present",
            "in_primary140",
            "in_genomovar173",
            "validated_automatic_assignment",
        ],
    ),
    (
        "definition_groups",
        "genomovar_id",
        &["genomes", "representative_accession"],
    ),
    (
        "definition_memberships",
        "accession",
        &[
            "project_species",
            "project_subspecies_proposal",
            "genomovar_id",
            "genomovar_version",
            "genomovar_rule",
        ],
    ),
    (
        "definition_references",
        "accession",
        &[
            "genomovar_id",
            "definition_version",
            "definition_rule",
            "group",
            "partition",
            "chromosome_fasta",
            "chromosome_sha256",
            "is_type",
            "qc_pass",
            "quality_flags",
        ],
    ),
    (
        "development_context",
        "accession",
        &["reference_in_proposed_package"],
    ),
    (
        "genomovar_crosswalk",
        "genomovar_id",
        &[
            "parent_concept_id",
            "version",
            "historical_rule",
            "definition_member_count",
            "representative_accession",
        ],
    ),
    (
        "historical_types",
        "historical_taxon_name",
        &[
            "represented_name_bearing_accessions",
            "current_project_species",
            "current_project_stratum",
        ],
    ),
    (
        "lineage_registry",
        "concept_id",
        &[
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
        "concept_id",
        &[
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
        "accession",
        &[
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
        ],
    ),
];

/// One parsed table: its columns in header order, and its rows in file order.
pub(super) struct Table {
    pub(super) name: &'static str,
    pub(super) columns: Vec<String>,
    pub(super) rows: Vec<Row>,
    key: &'static str,
}

impl Table {
    pub(super) fn has_column(&self, column: &str) -> bool {
        self.columns.iter().any(|c| c == column)
    }

    /// The rows by key, in key order. Keys were checked unique when the table was parsed.
    pub(super) fn by_key(&self) -> BTreeMap<&str, &Row> {
        self.rows
            .iter()
            .map(|row| (cell(row, self.key), row))
            .collect()
    }

    pub(super) fn keys(&self) -> BTreeSet<&str> {
        self.rows.iter().map(|row| cell(row, self.key)).collect()
    }
}

pub(super) struct Tables(BTreeMap<&'static str, Table>);

impl Tables {
    pub(super) fn load(
        assets: &[Asset],
        held: &BTreeMap<&str, Vec<u8>>,
    ) -> Result<Self, PackageError> {
        let mut tables = BTreeMap::new();
        for (name, key, columns) in TABLES {
            let asset = role_asset(assets, &format!("table:{name}"))?;
            let bytes = held
                .get(asset.path.as_str())
                .ok_or_else(|| inconsistent(format!("{} was not read", asset.path)))?;
            tables.insert(name, parse(name, key, bytes, columns)?);
        }
        Ok(Self(tables))
    }

    pub(super) fn get(&self, name: &str) -> &Table {
        &self.0[name]
    }
}

/// Parse a table under the research loader's rules: a header with no repeated column that holds
/// the key and every column read, rows with as many fields as the header, and unique keys. A
/// quote or a lone carriage return is refused rather than given a quoting rule the source may
/// not have meant; the frozen tables contain neither.
fn parse(
    name: &'static str,
    key: &'static str,
    bytes: &[u8],
    required: &[&str],
) -> Result<Table, PackageError> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| inconsistent(format!("{name} is not UTF-8")))?;
    if text.contains('"') {
        return Err(inconsistent(format!("{name} contains a quote character")));
    }
    let mut lines = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line));
    let header = lines
        .next()
        .filter(|line| !line.is_empty())
        .ok_or_else(|| inconsistent(format!("{name} has no header")))?;
    let columns: Vec<String> = header.split('\t').map(str::to_string).collect();
    let distinct: BTreeSet<&str> = columns.iter().map(String::as_str).collect();
    if distinct.len() != columns.len() {
        return Err(inconsistent(format!("{name} repeats a column")));
    }
    if let Some(missing) = std::iter::once(&key)
        .chain(required)
        .find(|column| !distinct.contains(**column))
    {
        return Err(inconsistent(format!("{name} has no {missing} column")));
    }

    let mut rows = Vec::new();
    let mut lines = lines.peekable();
    while let Some(line) = lines.next() {
        if line.is_empty() && lines.peek().is_none() {
            break;
        }
        if line.contains('\r') {
            return Err(inconsistent(format!("{name} has a stray carriage return")));
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != columns.len() {
            return Err(inconsistent(format!(
                "{name} has a row of {} fields, not {}",
                fields.len(),
                columns.len()
            )));
        }
        rows.push(
            columns
                .iter()
                .zip(fields)
                .map(|(column, field)| (column.clone(), field.to_string()))
                .collect::<Row>(),
        );
    }

    let mut seen = BTreeSet::new();
    for row in &rows {
        let value = cell(row, key);
        if value.is_empty() || value.trim() != value {
            return Err(inconsistent(format!("{name} has an empty or padded {key}")));
        }
        if !seen.insert(value) {
            return Err(inconsistent(format!("{name} repeats {key} {value}")));
        }
    }
    Ok(Table {
        name,
        columns,
        rows,
        key,
    })
}

pub(super) fn cell<'a>(row: &'a Row, column: &str) -> &'a str {
    row.get(column).map_or("", String::as_str)
}
