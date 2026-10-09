//! The v1 species decision over saved evidence.
//!
//! Pure logic: evidence in, decision out. No terminal, file or process access, so a later
//! browser or GUI front end can call it unchanged.
//!
//! This is the tracer. It applies the frozen rank rule for the species call (nearest reference,
//! ANI and aligned-fraction gates, a measured competitor from another parent). The other v1
//! withholding checks and the subspecies rank belong to later tickets. The JSON lists the
//! checks that were not applied, so an `assigned` here is never read as a complete v1 result.

use std::collections::HashSet;

use serde_json::{json, Value};

use crate::evidence::{Percent, Provenance, Reference, SavedEvidence, Taxon};

/// Identifier of the approved v1 calling policy, as recorded in the frozen 0.2.0 reports.
pub const V1_POLICY_ID: &str = "conservative-assembly-v1";
/// The reason the frozen 0.2.0 reports give for an assigned species.
const ASSIGNED_REASON: &str = "conditional_assembly_assignment";
/// Frozen v1 withholding reasons that this build does not check yet.
const V1_CHECKS_NOT_APPLIED: [&str; 4] = [
    "two_parent_support_alert",
    "unexplained_residual_sequence",
    "coverage_method_disagreement",
    "external_problem_warning",
];

/// Numerical gates of a calling policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    id: &'static str,
    species_ani: Percent,
    species_both_af: Percent,
}

impl Policy {
    /// The approved v1 gates: species ANI of at least 96% and both aligned fractions of at least 80%.
    pub fn v1() -> Self {
        Policy {
            id: V1_POLICY_ID,
            species_ani: Percent::from_whole(96),
            species_both_af: Percent::from_whole(80),
        }
    }

    pub fn id(&self) -> &'static str {
        self.id
    }

    fn meets_species_ani(&self, ani: Percent) -> bool {
        ani >= self.species_ani
    }

    fn passes(&self, reference: &Reference) -> bool {
        self.meets_species_ani(reference.ani) && reference.min_af() >= self.species_both_af
    }
}

/// Why a species was not assigned. The names are the frozen 0.2.0 reason strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WithholdReason {
    NoEstimate,
    DifferentSpeciesNearestTie,
    NearestReferenceHasNoSpecies,
    SpeciesCompetitorUnmeasured,
    NearestBelowSpeciesAni,
    SpeciesCoverageNotMet,
}

impl WithholdReason {
    pub fn as_str(self) -> &'static str {
        match self {
            WithholdReason::NoEstimate => "no_estimate",
            WithholdReason::DifferentSpeciesNearestTie => "different_species_nearest_tie",
            WithholdReason::NearestReferenceHasNoSpecies => "nearest_reference_has_no_species",
            WithholdReason::SpeciesCompetitorUnmeasured => "species_competitor_unmeasured",
            WithholdReason::NearestBelowSpeciesAni => "nearest_below_species_ANI",
            WithholdReason::SpeciesCoverageNotMet => "species_coverage_not_met",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpeciesCall {
    Assigned {
        species: Taxon,
        /// The reference that satisfied the ANI and aligned-fraction gates.
        witness: Reference,
    },
    Withheld(WithholdReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    policy: &'static str,
    provenance: Provenance,
    species: SpeciesCall,
}

/// What distinguishes one parent taxon from another when looking for competitors. A reference
/// without a species falls back to its lineage, then to itself, so unmapped references are
/// never silently merged into one taxon.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Parent<'a> {
    Species(&'a str),
    Lineage(&'a str),
    Untyped(&'a str),
}

fn parent(reference: &Reference) -> Parent<'_> {
    match (&reference.species, &reference.lineage_id) {
        (Some(species), _) => Parent::Species(&species.id),
        (None, Some(lineage)) => Parent::Lineage(lineage),
        (None, None) => Parent::Untyped(&reference.id),
    }
}

fn call_species(references: &[Reference], policy: &Policy) -> SpeciesCall {
    use WithholdReason::*;

    let Some(nearest) = references.first() else {
        return SpeciesCall::Withheld(NoEstimate);
    };
    let nearest_parent = parent(nearest);
    let tied_parents: HashSet<Parent> = references
        .iter()
        .take_while(|r| r.ani == nearest.ani)
        .map(parent)
        .collect();
    if tied_parents.len() > 1 {
        return SpeciesCall::Withheld(DifferentSpeciesNearestTie);
    }
    let Some(species) = &nearest.species else {
        return SpeciesCall::Withheld(NearestReferenceHasNoSpecies);
    };
    if references.iter().all(|r| parent(r) == nearest_parent) {
        return SpeciesCall::Withheld(SpeciesCompetitorUnmeasured);
    }
    if !policy.meets_species_ani(nearest.ani) {
        return SpeciesCall::Withheld(NearestBelowSpeciesAni);
    }
    // Species are compared by ID; validation guarantees an ID has one display name.
    let witness = references
        .iter()
        .find(|r| r.species.as_ref().map(|s| &s.id) == Some(&species.id) && policy.passes(r));
    match witness {
        Some(witness) => SpeciesCall::Assigned {
            species: species.clone(),
            witness: witness.clone(),
        },
        None => SpeciesCall::Withheld(SpeciesCoverageNotMet),
    }
}

/// Decide the v1 species call for validated saved evidence.
pub fn decide(evidence: &SavedEvidence, policy: &Policy) -> Decision {
    Decision {
        policy: policy.id(),
        provenance: evidence.provenance().clone(),
        species: call_species(evidence.references(), policy),
    }
}

impl Decision {
    /// The authoritative JSON for this decision.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.to_value()).expect("a JSON value always serialises")
    }

    fn to_value(&self) -> Value {
        json!({
            "schema": "terobacter.species-call/0",
            "implementation": {
                "name": env!("CARGO_PKG_NAME"),
                "version": env!("CARGO_PKG_VERSION"),
            },
            "policy": { "id": self.policy },
            "input": {
                "kind": "assembly",
                "evidence_schema": crate::evidence::EVIDENCE_SCHEMA,
                "engine_version": self.provenance.engine_version,
                "reference_package_sha256": self.provenance.reference_package_sha256,
                "query_sha256": self.provenance.query_sha256,
            },
            "v1_checks_not_applied": V1_CHECKS_NOT_APPLIED,
            "species": self.species_value(),
            "subspecies": {
                "status": "not_evaluated",
                "reason": "subspecies_rank_not_implemented_in_this_build",
            },
        })
    }

    fn species_value(&self) -> Value {
        match &self.species {
            SpeciesCall::Assigned { species, witness } => json!({
                "status": "assigned",
                "reason": ASSIGNED_REASON,
                "id": species.id,
                "name": species.name,
                "witness": {
                    "reference_id": witness.id,
                    "ani_percent": witness.ani.as_f64(),
                    "aligned_fraction_query_percent": witness.af_query.as_f64(),
                    "aligned_fraction_reference_percent": witness.af_reference.as_f64(),
                },
            }),
            SpeciesCall::Withheld(reason) => json!({
                "status": "unresolved",
                "reason": reason.as_str(),
                "id": null,
                "name": null,
            }),
        }
    }
}
