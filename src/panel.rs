//! The primary reference panel: every reference the query competes against, including the ones
//! skani did not return. v1 method diagnostics use it to bind reference hashes; v2 needs it
//! because a positive rescue must assess the whole panel. Pure: it takes text, never a path.
use std::collections::{BTreeSet, HashSet};

use serde::Deserialize;

use crate::evidence::{invalid, is_sha256, EvidenceError, SavedEvidence, Taxon};

const INVENTORY_SCHEMA: &str = "1.0-shadow-primary-inventory";

/// One primary reference, whether or not skani returned it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelReference {
    pub id: String,
    pub source_sha256: String,
    pub species: Option<Taxon>,
    pub subspecies: Option<Taxon>,
    pub lineage_id: Option<String>,
    pub reference_role: String,
    pub name_status: String,
}

/// The primary panel from a checked reference package, bound to one saved-evidence record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Panel {
    references: Vec<PanelReference>,
    evidence_sha256: String,
    source_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInventory {
    schema_version: String,
    package_sha256: String,
    component_id: String,
    #[allow(dead_code)] // carried for provenance only
    representation: serde_json::Value,
    references: Vec<RawPanelReference>,
}

#[derive(Deserialize)]
struct RawPanelReference {
    reference_id: String,
    source_sha256: String,
    species_id: String,
    species_name: String,
    subspecies_id: String,
    subspecies_name: String,
    lineage_id: String,
    name_status: String,
    reference_role: String,
}

fn taxon(id: &str, name: &str, reference: &str) -> Result<Option<Taxon>, EvidenceError> {
    match (id.is_empty(), name.is_empty()) {
        (true, true) => Ok(None),
        (false, false) => Ok(Some(Taxon {
            id: id.to_owned(),
            name: name.to_owned(),
        })),
        _ => invalid(format!(
            "{reference}: taxon ID and name must both be present or absent"
        )),
    }
}

impl Panel {
    /// Build the complete primary panel from an authenticated package and bind its metadata
    /// to the original, unmasked saved evidence.
    pub fn from_package(
        package: &crate::reference::VerifiedPackage,
        evidence: &SavedEvidence,
    ) -> Result<Self, EvidenceError> {
        package
            .check_evidence(evidence)
            .map_err(|error| EvidenceError::Invalid(error.to_string()))?;
        let component = package
            .component("primary_whole")
            .ok_or_else(|| EvidenceError::Invalid("missing primary component".into()))?;
        let references: Vec<_> = component.references.iter().map(|r| {
            let species = package.concept(&r.species_id).expect("verified species");
            let subspecies = r.subspecies_id.as_deref().and_then(|id| package.concept(id));
            serde_json::json!({"reference_id": r.reference_id, "source_sha256": r.source.sha256,
                "species_id": species.id, "species_name": species.display_name,
                "subspecies_id": subspecies.map_or("", |c| c.id.as_str()),
                "subspecies_name": subspecies.map_or("", |c| c.display_name.as_str()),
                "lineage_id": "", "reference_role": if r.type_anchor { "type_anchor" } else { "diversity" },
                "name_status": subspecies.unwrap_or(species).nomenclature_status})
        }).collect();
        let inventory = serde_json::json!({"schema_version": INVENTORY_SCHEMA,
            "package_sha256": evidence.provenance().reference_package_sha256,
            "component_id": "primary_whole", "representation": {"source": "authenticated_package"},
            "references": references});
        Self::from_json(&inventory.to_string(), evidence)
    }

    /// Parse a primary inventory and check it against `evidence`: the same package, the same
    /// reference count, and identical metadata for every reference skani returned.
    pub fn from_json(text: &str, evidence: &SavedEvidence) -> Result<Self, EvidenceError> {
        let raw: RawInventory = serde_json::from_str(text)?;
        if raw.schema_version != INVENTORY_SCHEMA
            || raw.component_id != "primary_whole"
            || raw.package_sha256 != evidence.provenance().reference_package_sha256
        {
            return invalid("the inventory is not the primary panel of the evidence's package");
        }
        if raw.references.len() != evidence.reference_count() {
            return invalid("inventory count mismatch");
        }
        let mut ids = HashSet::new();
        let mut references = Vec::with_capacity(raw.references.len());
        for r in &raw.references {
            let id = &r.reference_id;
            if id.trim().is_empty() || !ids.insert(id.as_str()) || !is_sha256(&r.source_sha256) {
                return invalid(format!("{id}: invalid or duplicate inventory reference"));
            }
            let species = taxon(&r.species_id, &r.species_name, id)?;
            let subspecies = taxon(&r.subspecies_id, &r.subspecies_name, id)?;
            if subspecies.is_some() && species.is_none() {
                return invalid(format!("{id}: a subspecies needs its species"));
            }
            if species.is_none() && r.lineage_id.is_empty() && r.reference_role != "context_only" {
                return invalid(format!("{id}: an unmapped reference must be context_only"));
            }
            references.push(PanelReference {
                id: id.clone(),
                source_sha256: r.source_sha256.clone(),
                species,
                subspecies,
                lineage_id: (!r.lineage_id.is_empty()).then(|| r.lineage_id.clone()),
                reference_role: r.reference_role.clone(),
                name_status: r.name_status.clone(),
            });
        }
        for hit in evidence.references() {
            let Some(r) = references.iter().find(|r| r.id == hit.id) else {
                return invalid(format!("{}: reference outside primary inventory", hit.id));
            };
            if r.species != hit.species
                || r.subspecies != hit.subspecies
                || r.lineage_id != hit.lineage_id
                || r.reference_role != hit.reference_role
                || r.name_status != hit.name_status
            {
                return invalid(format!("{}: inventory concept/name/role mismatch", hit.id));
            }
        }
        Ok(Panel {
            references,
            evidence_sha256: evidence.source_sha256().to_owned(),
            source_sha256: crate::evidence::sha256_hex(text.as_bytes()),
        })
    }

    pub fn references(&self) -> &[PanelReference] {
        &self.references
    }

    pub fn get(&self, id: &str) -> Option<&PanelReference> {
        self.references.iter().find(|r| r.id == id)
    }

    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    pub(crate) fn evidence_sha256(&self) -> &str {
        &self.evidence_sha256
    }

    /// Remove masked references from the evidence rows, its panel counts and this panel. The
    /// panel must be bound to `evidence`, every masked reference must be in it, and none may be
    /// named twice. Returns the masked evidence and panel with the sorted masked IDs.
    pub(crate) fn mask(
        &self,
        evidence: &SavedEvidence,
        mask: &[String],
    ) -> Result<(SavedEvidence, Panel, BTreeSet<String>), EvidenceError> {
        if self.evidence_sha256() != evidence.source_sha256() {
            return invalid("the primary inventory was checked against different evidence");
        }
        let mut set = BTreeSet::new();
        for id in mask {
            if self.get(id).is_none() {
                return invalid(format!(
                    "{id}: masked reference is not in the primary panel"
                ));
            }
            if !set.insert(id.as_str()) {
                return invalid(format!("{id}: reference masked twice"));
            }
        }
        let unreturned = set
            .iter()
            .filter(|id| !evidence.references().iter().any(|r| r.id == **id))
            .count();
        Ok((
            evidence.without_references(&set, unreturned),
            self.without(&set),
            set.iter().map(|id| (*id).to_owned()).collect(),
        ))
    }

    fn without(&self, mask: &BTreeSet<&str>) -> Panel {
        Panel {
            references: self
                .references
                .iter()
                .filter(|r| !mask.contains(r.id.as_str()))
                .cloned()
                .collect(),
            ..self.clone()
        }
    }
}
