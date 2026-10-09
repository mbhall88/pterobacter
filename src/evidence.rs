//! Saved evidence: the part of the 0.2.0 development-evidence JSON that the v1 decision reads.
//!
//! This module is pure data and validation. It takes text, never a path, so any front end
//! (command line, browser, GUI) can supply the evidence however it likes.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Deserializer};

/// Schema identifier of the frozen 0.2.0 saved-evidence records this module reads.
pub const EVIDENCE_SCHEMA: &str = "2.0.0-dev-evidence";
const RECORD_TYPE: &str = "development_evidence_only";
const PRIMARY_COMPONENT: &str = "primary_whole";
const ASSEMBLY: &str = "assembly";
const ESTIMATES_RETURNED: &str = "estimates_returned";
const NO_ESTIMATE_RETURNED: &str = "no_estimate_returned";
const UNASSIGNED_STATES: [&str; 3] = ["not_evaluated", "unresolved", "unassigned"];

#[derive(Debug, thiserror::Error)]
pub enum EvidenceError {
    #[error("saved evidence is not JSON of the expected shape: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid saved evidence: {0}")]
    Invalid(String),
}

fn invalid<T>(message: impl Into<String>) -> Result<T, EvidenceError> {
    Err(EvidenceError::Invalid(message.into()))
}

/// A percentage held as an integer number of hundredths of a percent.
///
/// skani reports two decimals and decisions use exactly those emitted decimals, so
/// comparisons here are exact integer comparisons rather than floating-point ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Percent(u32);

impl Percent {
    pub const fn from_whole(percent: u32) -> Self {
        Percent(percent * 100)
    }

    /// For display only; never compare these values.
    pub fn as_f64(self) -> f64 {
        f64::from(self.0) / 100.0
    }

    fn parse(text: &str) -> Option<Self> {
        let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
        let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
        if whole.is_empty() || whole.len() > 3 || fraction.len() > 2 {
            return None;
        }
        if !digits(whole) || !digits(fraction) {
            return None;
        }
        let whole: u32 = whole.parse().ok()?;
        let fraction: u32 = format!("{fraction:0<2}").parse().ok()?;
        let hundredths = whole * 100 + fraction;
        (hundredths <= 10_000).then_some(Percent(hundredths))
    }
}

impl<'de> Deserialize<'de> for Percent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let number = serde_json::Number::deserialize(deserializer)?;
        Percent::parse(&number.to_string()).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "{number} is not a percentage between 0 and 100 with at most two decimals"
            ))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Taxon {
    pub id: String,
    pub name: String,
}

/// One query-versus-reference comparison from the primary whole-assembly panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub id: String,
    pub species: Option<Taxon>,
    pub lineage_id: Option<String>,
    pub ani: Percent,
    pub af_query: Percent,
    pub af_reference: Percent,
}

impl Reference {
    /// The smaller of the two directional aligned fractions.
    pub fn min_af(&self) -> Percent {
        self.af_query.min(self.af_reference)
    }
}

/// What the evidence says about where it came from. A decision given a verified reference
/// package first checks these bindings with [`crate::reference::VerifiedPackage::check_binding`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub engine_version: String,
    pub reference_package_sha256: String,
    pub query_sha256: String,
}

/// Validated saved evidence, with references in rank order (highest ANI first).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedEvidence {
    references: Vec<Reference>,
    provenance: Provenance,
}

impl SavedEvidence {
    /// Parse and validate a 0.2.0 saved-evidence record.
    ///
    /// Rejects anything that could silently change a decision: another schema, a failed
    /// search, rows that are not canonically ranked, or counts that show a truncated panel.
    pub fn from_json(text: &str) -> Result<Self, EvidenceError> {
        let raw: RawEvidence = serde_json::from_str(text)?;
        raw.validate()
    }

    pub fn references(&self) -> &[Reference] {
        &self.references
    }

    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }
}

#[derive(Deserialize)]
struct RawEvidence {
    schema_version: String,
    record_type: String,
    input_kind: String,
    units: RawUnits,
    assignment: RawAssignment,
    provenance: RawProvenance,
    evidence_status: String,
    reference_count: usize,
    raw_hit_count: usize,
    unique_hit_count: usize,
    not_returned_reference_count: usize,
    duplicate_rows_collapsed: usize,
    nearest_reference_ids: Vec<String>,
    ranked_reference_evidence: Vec<RawReference>,
}

#[derive(Deserialize)]
struct RawUnits {
    ani: String,
    aligned_fraction: String,
}

#[derive(Deserialize)]
struct RawAssignment {
    status: String,
}

#[derive(Deserialize)]
struct RawProvenance {
    input_kind: String,
    engine_version: String,
    reference_component_id: String,
    reference_package_sha256: String,
    query_sha256: String,
    search_exit_code: i64,
}

#[derive(Deserialize)]
struct RawReference {
    reference_id: String,
    species_id: String,
    species_name: String,
    lineage_id: String,
    ani_percent: Percent,
    aligned_fraction_query_percent: Percent,
    aligned_fraction_reference_percent: Percent,
    minimum_directional_af_percent: Percent,
}

fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

impl RawEvidence {
    fn validate(self) -> Result<SavedEvidence, EvidenceError> {
        if self.schema_version != EVIDENCE_SCHEMA || self.record_type != RECORD_TYPE {
            return invalid(format!(
                "expected schema {EVIDENCE_SCHEMA} ({RECORD_TYPE}), found {} ({})",
                self.schema_version, self.record_type
            ));
        }
        if self.input_kind != ASSEMBLY || self.provenance.input_kind != ASSEMBLY {
            return invalid("only assembly evidence is supported");
        }
        if self.units.ani != "percent" || self.units.aligned_fraction != "percent" {
            return invalid("ANI and aligned fraction must both be in percent");
        }
        if !UNASSIGNED_STATES.contains(&self.assignment.status.as_str()) {
            return invalid("saved evidence must carry an unassigned input state");
        }
        let provenance = &self.provenance;
        if provenance.search_exit_code != 0 {
            return invalid("only evidence from a successful search is accepted");
        }
        if provenance.reference_component_id != PRIMARY_COMPONENT {
            return invalid("species evidence requires the primary whole-assembly component");
        }
        if provenance.engine_version.is_empty() {
            return invalid("the search engine version is required");
        }
        if !is_sha256(&provenance.reference_package_sha256) || !is_sha256(&provenance.query_sha256)
        {
            return invalid("reference package and query must be bound by SHA-256 digests");
        }

        let mut references = Vec::with_capacity(self.ranked_reference_evidence.len());
        for row in &self.ranked_reference_evidence {
            references.push(row.to_reference()?);
        }
        check_ranking(&references)?;
        check_reference_identities(&references)?;
        check_panel_summary(&self, &references)?;

        Ok(SavedEvidence {
            references,
            provenance: Provenance {
                engine_version: self.provenance.engine_version,
                reference_package_sha256: self.provenance.reference_package_sha256,
                query_sha256: self.provenance.query_sha256,
            },
        })
    }
}

impl RawReference {
    fn to_reference(&self) -> Result<Reference, EvidenceError> {
        let id = &self.reference_id;
        if id.is_empty() {
            return invalid("every reference needs an identifier");
        }
        let zero = Percent(0);
        if self.aligned_fraction_query_percent == zero
            || self.aligned_fraction_reference_percent == zero
        {
            return invalid(format!(
                "{id}: a returned ANI requires a positive aligned fraction"
            ));
        }
        let reference = Reference {
            id: id.clone(),
            species: match (self.species_id.is_empty(), self.species_name.is_empty()) {
                (true, true) => None,
                (false, false) => Some(Taxon {
                    id: self.species_id.clone(),
                    name: self.species_name.clone(),
                }),
                _ => {
                    return invalid(format!(
                        "{id}: species ID and name must both be present or absent"
                    ))
                }
            },
            lineage_id: (!self.lineage_id.is_empty()).then(|| self.lineage_id.clone()),
            ani: self.ani_percent,
            af_query: self.aligned_fraction_query_percent,
            af_reference: self.aligned_fraction_reference_percent,
        };
        if reference.min_af() != self.minimum_directional_af_percent {
            return invalid(format!(
                "{id}: minimum aligned fraction does not match its parts"
            ));
        }
        Ok(reference)
    }
}

fn check_ranking(references: &[Reference]) -> Result<(), EvidenceError> {
    let ranked = references
        .windows(2)
        .all(|pair| (Reverse(pair[0].ani), &pair[0].id) < (Reverse(pair[1].ani), &pair[1].id));
    if ranked {
        Ok(())
    } else {
        invalid("references must be ranked by descending ANI, then reference ID, without repeats")
    }
}

/// Reference IDs are unique, and one species ID always carries the same display name, so the
/// decision can compare species by ID alone.
fn check_reference_identities(references: &[Reference]) -> Result<(), EvidenceError> {
    let mut ids = HashSet::new();
    let mut species_names: HashMap<&str, &str> = HashMap::new();
    for reference in references {
        if !ids.insert(reference.id.as_str()) {
            return invalid(format!("{}: reference IDs must be unique", reference.id));
        }
        if let Some(taxon) = &reference.species {
            let name = species_names.entry(&taxon.id).or_insert(&taxon.name);
            if *name != taxon.name {
                return invalid(format!(
                    "{}: species {} appears with conflicting names",
                    reference.id, taxon.id
                ));
            }
        }
    }
    Ok(())
}

/// The summary fields must agree with the rows supplied, so a truncated or edited panel
/// cannot quietly drop a competitor.
fn check_panel_summary(raw: &RawEvidence, references: &[Reference]) -> Result<(), EvidenceError> {
    let returned = references.len();
    let consistent = raw.unique_hit_count == returned
        && raw.reference_count > 0
        && raw.reference_count.checked_sub(returned) == Some(raw.not_returned_reference_count)
        && raw.raw_hit_count.checked_sub(returned) == Some(raw.duplicate_rows_collapsed);
    if !consistent {
        return invalid(
            "reference counts disagree with the rows supplied; the panel may be truncated",
        );
    }
    let nearest: Vec<&str> = references
        .iter()
        .take_while(|r| r.ani == references[0].ani)
        .map(|r| r.id.as_str())
        .collect();
    if raw.nearest_reference_ids != nearest {
        return invalid("nearest reference IDs do not match the highest-ANI rows");
    }
    let status = if returned > 0 {
        ESTIMATES_RETURNED
    } else {
        NO_ESTIMATE_RETURNED
    };
    if raw.evidence_status != status {
        return invalid("evidence status does not match the rows supplied");
    }
    Ok(())
}
