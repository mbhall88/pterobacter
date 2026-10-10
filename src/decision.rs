//! Frozen conservative-assembly-v1 decisions from validated, saved skani evidence.
//! This module has no terminal, filesystem or process access.
use std::collections::HashSet;

use serde_json::{json, Value};

use crate::evidence::{Percent, Reference, SavedEvidence, Taxon};
use crate::reference::{PackageError, VerifiedPackage};

pub const V1_POLICY_ID: &str = "conservative-assembly-v1";
const ASSIGNED: &str = "conditional_assembly_assignment";

/// The frozen policy uses emitted hundredths, including the one-point subspecies margin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    id: &'static str,
    species_ani: Percent,
    both_af: Percent,
    subspecies_ani: Percent,
    subspecies_margin: i64,
    alert_ani: Percent,
}
impl Policy {
    pub fn v1() -> Self {
        Self {
            id: V1_POLICY_ID,
            species_ani: Percent::from_whole(96),
            both_af: Percent::from_whole(80),
            subspecies_ani: Percent::from_whole(98),
            subspecies_margin: 100,
            alert_ani: Percent::from_whole(96),
        }
    }
    /// The v1 skani rules at other ANI gates, for v2's ordinary skani route. Every other gate,
    /// including AF 80 and the one-point margin, is unchanged.
    pub(crate) fn skani_route(
        id: &'static str,
        species_ani: Percent,
        subspecies_ani: Percent,
        alert_ani: Percent,
    ) -> Self {
        Self {
            id,
            species_ani,
            subspecies_ani,
            alert_ani,
            ..Self::v1()
        }
    }
    pub fn id(&self) -> &'static str {
        self.id
    }
    /// The numerical minima, as the decimal text v2 reports in its own `thresholds`.
    pub fn thresholds_json(&self) -> Value {
        let text = |p: Percent| {
            p.to_string()
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_owned()
        };
        let margin =
            Percent::from_hundredths(u32::try_from(self.subspecies_margin).expect("margin"));
        json!({"species_ani": text(self.species_ani), "subspecies_ani": text(self.subspecies_ani),
            "both_af_minimum": text(self.both_af),
            "subspecies_margin_percentage_points": text(margin),
            "two_parent_alert_ani": text(self.alert_ani), "supplied": null})
    }
    fn passes(&self, r: &Reference, ani: Percent) -> bool {
        r.ani >= ani && r.min_af() >= self.both_af
    }
}

/// Authoritative assignment plus separate raw and candidate evidence and every veto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    value: Value,
}
impl Decision {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.value).expect("a JSON value always serialises")
    }
    pub fn to_value(&self) -> &Value {
        &self.value
    }
    pub(crate) fn from_value(value: Value) -> Self {
        Self { value }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Parent<'a> {
    Species(&'a str),
    Lineage(&'a str),
    Untyped(&'a str),
}
fn parent(r: &Reference) -> Parent<'_> {
    if let Some(s) = &r.species {
        Parent::Species(&s.id)
    } else if let Some(l) = &r.lineage_id {
        Parent::Lineage(l)
    } else {
        Parent::Untyped(&r.id)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Group<'a> {
    Subspecies(&'a str),
    Parent(Parent<'a>),
}
fn group(r: &Reference) -> Group<'_> {
    r.subspecies
        .as_ref()
        .map_or_else(|| Group::Parent(parent(r)), |s| Group::Subspecies(&s.id))
}
fn group_value(r: &Reference) -> Value {
    let (kind, id) = match group(r) {
        Group::Subspecies(id) => ("subspecies", Some(id)),
        Group::Parent(Parent::Species(id)) => ("species_only", Some(id)),
        Group::Parent(Parent::Lineage(id)) => ("lineage", Some(id)),
        Group::Parent(Parent::Untyped(_)) => ("untyped_reference", None),
    };
    json!({"kind": kind, "concept_id": id,
        "species_id": r.species.as_ref().map(|s| &s.id),
        "subspecies_id": r.subspecies.as_ref().map(|s| &s.id),
        "name": r.subspecies.as_ref().or(r.species.as_ref()).map(|s| &s.name)})
}
fn hit(r: &Reference) -> Value {
    json!({"reference_id": r.id, "species_id": r.species.as_ref().map(|s| &s.id),
        "subspecies_id": r.subspecies.as_ref().map(|s| &s.id), "lineage_id": r.lineage_id,
        "reference_role": r.reference_role, "reference_name_status": r.name_status,
        "ani_percent": r.ani.as_f64(), "aligned_fraction_query_percent": r.af_query.as_f64(),
        "aligned_fraction_reference_percent": r.af_reference.as_f64()})
}
fn best(rows: &[&Reference]) -> Value {
    rows.first().map_or(Value::Null, |top| json!({"ani_percent": top.ani.as_f64(),
        "tied_references": rows.iter().take_while(|r| r.ani == top.ani).map(|r| hit(r)).collect::<Vec<_>>() }))
}
fn gap(a: &Reference, b: &Reference) -> Value {
    json!(a.ani.gap(b.ani) as f64 / 100.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RankReason {
    Candidate,
    NoEstimate,
    DifferentSpeciesNearestTie,
    NearestReferenceHasNoSpecies,
    SpeciesCompetitorUnmeasured,
    NearestBelowSpeciesAni,
    SpeciesCoverageNotMet,
    ParentSpeciesUnresolved,
    WithinParentNearestGroupTie,
    NearestReferenceHasNoSubspecies,
    NearestGroupBelowSubspeciesAni,
    SubspeciesCoverageNotMet,
    WithinParentCompetitorUnmeasured,
    GlobalGroupCompetitorUnmeasured,
    GlobalGroupMarginBelowGuide,
    QualifyingReferenceMarginBelowGuide,
}
impl RankReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "diagnostic_candidate_only",
            Self::NoEstimate => "no_estimate",
            Self::DifferentSpeciesNearestTie => "different_species_nearest_tie",
            Self::NearestReferenceHasNoSpecies => "nearest_reference_has_no_species",
            Self::SpeciesCompetitorUnmeasured => "species_competitor_unmeasured",
            Self::NearestBelowSpeciesAni => "nearest_below_species_ANI",
            Self::SpeciesCoverageNotMet => "species_coverage_not_met",
            Self::ParentSpeciesUnresolved => "parent_species_unresolved",
            Self::WithinParentNearestGroupTie => "within_parent_nearest_group_tie",
            Self::NearestReferenceHasNoSubspecies => "nearest_reference_has_no_subspecies",
            Self::NearestGroupBelowSubspeciesAni => "nearest_group_below_subspecies_ANI",
            Self::SubspeciesCoverageNotMet => "subspecies_coverage_not_met",
            Self::WithinParentCompetitorUnmeasured => "within_parent_competitor_unmeasured",
            Self::GlobalGroupCompetitorUnmeasured => "global_group_competitor_unmeasured",
            Self::GlobalGroupMarginBelowGuide => "global_group_margin_below_guide",
            Self::QualifyingReferenceMarginBelowGuide => "qualifying_reference_margin_below_guide",
        }
    }
}

struct RankCall<'a> {
    candidate: Option<&'a Taxon>,
    witness: Option<&'a Reference>,
    reason: RankReason,
    evidence: Value,
}
impl RankCall<'_> {
    fn reason(&self) -> &str {
        self.reason.as_str()
    }
}
fn species_rank<'a>(rows: &[&'a Reference], policy: &Policy) -> RankCall<'a> {
    let mut out = json!({"candidate_id": null, "candidate_name": null, "reason": "no_estimate",
        "raw_nearest": best(rows), "qualifying_witness": null, "best_other_parent": null,
        "competing_parents": []});
    let mut candidate = None;
    let mut witness = None;
    let mut reason = RankReason::NoEstimate;
    if let Some(top) = rows.first() {
        let other: Vec<_> = rows
            .iter()
            .copied()
            .filter(|r| parent(r) != parent(top))
            .collect();
        out["best_other_parent"] = best(&other);
        let mut seen = HashSet::new();
        out["competing_parents"] = other.iter().filter(|r| seen.insert(parent(r))).map(|r| {
            let members: Vec<_> = other.iter().copied().filter(|m| parent(m) == parent(r)).collect();
            let (kind, id) = match parent(r) { Parent::Species(id) => ("species", Some(id)),
                Parent::Lineage(id) => ("lineage", Some(id)), Parent::Untyped(_) => ("untyped_reference", None) };
            json!({"kind": kind, "concept_id": id, "returned_reference_count": members.len(), "best": best(&members)})
        }).collect();
        let tied: HashSet<_> = rows
            .iter()
            .take_while(|r| r.ani == top.ani)
            .map(|r| parent(r))
            .collect();
        reason = if tied.len() > 1 {
            RankReason::DifferentSpeciesNearestTie
        } else if top.species.is_none() {
            RankReason::NearestReferenceHasNoSpecies
        } else if other.is_empty() {
            RankReason::SpeciesCompetitorUnmeasured
        } else if top.ani < policy.species_ani {
            RankReason::NearestBelowSpeciesAni
        } else {
            witness = rows
                .iter()
                .copied()
                .find(|r| parent(r) == parent(top) && policy.passes(r, policy.species_ani));
            if witness.is_some() {
                candidate = top.species.as_ref();
                RankReason::Candidate
            } else {
                RankReason::SpeciesCoverageNotMet
            }
        };
        out["reason"] = reason.as_str().into();
        if let Some(s) = candidate {
            out["candidate_id"] = json!(s.id);
            out["candidate_name"] = json!(s.name);
        }
        if let Some(w) = witness {
            out["qualifying_witness"] = hit(w);
        }
    }
    RankCall {
        candidate,
        witness,
        reason,
        evidence: out,
    }
}
fn subspecies_rank<'a>(
    rows: &[&'a Reference],
    species: Option<&Taxon>,
    policy: &Policy,
) -> RankCall<'a> {
    let mut out = json!({"candidate_id": null, "candidate_name": null, "reason": "parent_species_unresolved",
        "raw_nearest_group": null, "raw_nearest_ani_percent": null,
        "ani_af_witness": null, "qualifying_witness": null,
        "best_within_parent_competitor": null, "best_global_group_competitor": null,
        "best_foreign_parent_competitor": null, "raw_global_gap_percentage_points": null,
        "raw_within_parent_gap_percentage_points": null, "qualifying_witness_gap_percentage_points": null,
        "competing_groups": []});
    let Some(species) = species else {
        return RankCall {
            candidate: None,
            witness: None,
            reason: RankReason::ParentSpeciesUnresolved,
            evidence: out,
        };
    };
    let ph: Vec<_> = rows
        .iter()
        .copied()
        .filter(|r| r.species.as_ref().is_some_and(|s| s.id == species.id))
        .collect();
    let top = ph[0];
    out["raw_nearest_group"] = group_value(top);
    out["raw_nearest_ani_percent"] = json!(top.ani.as_f64());
    let other: Vec<_> = rows
        .iter()
        .copied()
        .filter(|r| group(r) != group(top))
        .collect();
    let mut seen = HashSet::new();
    out["competing_groups"] = other.iter().filter(|r| seen.insert(group(r))).map(|r| {
        let members: Vec<_> = other.iter().copied().filter(|m| group(m) == group(r)).collect();
        json!({"group": group_value(r), "returned_reference_count": members.len(), "best": best(&members)})
    }).collect();
    let tied: HashSet<_> = ph
        .iter()
        .take_while(|r| r.ani == top.ani)
        .map(|r| group(r))
        .collect();
    let mut witness = None;
    let reason = if tied.len() > 1 {
        RankReason::WithinParentNearestGroupTie
    } else if top.subspecies.is_none() {
        RankReason::NearestReferenceHasNoSubspecies
    } else {
        let within: Vec<_> = ph
            .iter()
            .copied()
            .filter(|r| group(r) != group(top))
            .collect();
        let foreign: Vec<_> = rows
            .iter()
            .copied()
            .filter(|r| r.species.as_ref().is_none_or(|s| s.id != species.id))
            .collect();
        out["best_within_parent_competitor"] = best(&within);
        out["best_global_group_competitor"] = best(&other);
        out["best_foreign_parent_competitor"] = best(&foreign);
        if let Some(r) = within.first() {
            out["raw_within_parent_gap_percentage_points"] = gap(top, r);
        }
        if let Some(r) = other.first() {
            out["raw_global_gap_percentage_points"] = gap(top, r);
        }
        let eligible = ph
            .iter()
            .copied()
            .find(|r| group(r) == group(top) && policy.passes(r, policy.subspecies_ani));
        if top.ani < policy.subspecies_ani {
            RankReason::NearestGroupBelowSubspeciesAni
        } else if let Some(w) = eligible {
            out["ani_af_witness"] = hit(w);
            if let Some(r) = other.first() {
                out["qualifying_witness_gap_percentage_points"] = gap(w, r);
            }
            if within.is_empty() {
                RankReason::WithinParentCompetitorUnmeasured
            } else if other.is_empty() {
                RankReason::GlobalGroupCompetitorUnmeasured
            } else if top.ani.gap(other[0].ani) < policy.subspecies_margin {
                RankReason::GlobalGroupMarginBelowGuide
            } else if w.ani.gap(other[0].ani) < policy.subspecies_margin {
                RankReason::QualifyingReferenceMarginBelowGuide
            } else {
                witness = Some(w);
                RankReason::Candidate
            }
        } else {
            RankReason::SubspeciesCoverageNotMet
        }
    };
    out["reason"] = reason.as_str().into();
    let candidate = witness.and_then(|w| w.subspecies.as_ref());
    if let Some(s) = candidate {
        out["candidate_id"] = json!(s.id);
        out["candidate_name"] = json!(s.name);
    }
    if let Some(w) = witness {
        out["qualifying_witness"] = hit(w);
    }
    RankCall {
        candidate,
        witness,
        reason,
        evidence: out,
    }
}
fn near_boundary(rows: &[&Reference], policy: &Policy) -> Value {
    let Some(top) = rows.first() else {
        return Value::Null;
    };
    let Some(species) = &top.species else {
        return Value::Null;
    };
    let tied: Vec<_> = rows
        .iter()
        .copied()
        .take_while(|r| r.ani == top.ani)
        .collect();
    if top.ani < Percent::from_whole(95)
        || top.ani >= policy.species_ani
        || tied
            .iter()
            .any(|r| r.species.as_ref().is_none_or(|s| s.id != species.id))
    {
        return Value::Null;
    }
    let Some(w) = tied.iter().find(|r| r.min_af() >= policy.both_af) else {
        return Value::Null;
    };
    json!({"state": "closest_species_below_call_threshold", "is_call": false,
        "species_id": species.id, "species_name": species.name, "reference_id": w.id,
        "tied_reference_ids": tied.iter().map(|r| &r.id).collect::<Vec<_>>(), "ani_percent": w.ani.as_f64(),
        "af_query_percent": w.af_query.as_f64(), "af_reference_percent": w.af_reference.as_f64(),
        "band": {"ani_minimum": "95", "species_call_ani": "96", "both_af_minimum": "80"},
        "interpretation": "closest species, below the call threshold; not a call",
        "message": format!("closest species: {} at {}% ANI, below the 96% call threshold; not a call", species.name, json!(w.ani.as_f64()))})
}
fn rank_output(call: &RankCall<'_>, permitted: bool, reason: &str) -> Value {
    let candidate = call.candidate.filter(|_| permitted);
    let mut out = json!({"status": if candidate.is_some() { "assigned" } else { "unresolved" },
        "reason": reason, "id": candidate.map(|s| &s.id), "name": candidate.map(|s| &s.name)});
    if let Some(w) = call.witness.filter(|_| permitted) {
        out["witness"] = hit(w);
    }
    out
}

/// Apply v1 without a reference package; the reference is reported as not verified.
pub fn decide(evidence: &SavedEvidence, policy: &Policy) -> Decision {
    assess(evidence, policy, None)
}

/// Apply v1 to evidence bound to a verified reference package; other evidence is refused.
pub fn decide_bound(
    evidence: &SavedEvidence,
    package: &VerifiedPackage,
    policy: &Policy,
) -> Result<Decision, PackageError> {
    package.check_evidence(evidence)?;
    Ok(assess(evidence, policy, Some(package)))
}

/// Apply v1. Missing optional assessments are explicit; they never establish purity.
fn assess(
    evidence: &SavedEvidence,
    policy: &Policy,
    package: Option<&VerifiedPackage>,
) -> Decision {
    let rows: Vec<_> = evidence.references().iter().collect();
    let species = species_rank(&rows, policy);
    let subspecies = subspecies_rank(&rows, species.candidate, policy);
    let triggers: Vec<_> = rows.iter().filter(|r| r.species.is_some()
        && r.ani >= policy.alert_ani && r.af_reference >= policy.both_af)
        .map(|r| json!({"reference_id": r.id, "species_id": r.species.as_ref().map(|s| &s.id),
            "ani_percent": r.ani.as_f64(), "af_query_percent": r.af_query.as_f64(), "af_reference_percent": r.af_reference.as_f64()})).collect();
    let alert = triggers
        .iter()
        .map(|t| t["species_id"].as_str())
        .collect::<HashSet<_>>()
        .len()
        >= 2;
    let incomplete = species.reason == RankReason::SpeciesCompetitorUnmeasured;
    let unexplained = evidence.scope()["outcome"] == "unexplained_after_checks";
    let method_block = evidence
        .diagnostics()
        .iter()
        .any(|d| d["gate_disagreement"] == true);
    let mut reasons = Vec::new();
    if species.candidate.is_none() && !incomplete {
        reasons.push(species.reason());
    }
    if alert {
        reasons.push("two_parent_support_alert");
    }
    if unexplained {
        reasons.push("unexplained_residual_sequence");
    }
    if method_block {
        reasons.push("coverage_method_disagreement");
    }
    if !evidence.problem_warnings().is_empty() {
        reasons.push("external_problem_warning");
    }
    let ineligible = !reasons.is_empty();
    if incomplete {
        reasons.push(species.reason());
    }
    let assigned = reasons.is_empty();
    let reason = if assigned {
        ASSIGNED.to_owned()
    } else if unexplained {
        "unexplained_residual_sequence".to_owned()
    } else {
        reasons.join(";")
    };
    let mut assignment = json!({"status": if assigned { "assigned" } else { "unresolved" }, "reason": reason,
        "species_id": species.candidate.filter(|_| assigned).map(|s| &s.id),
        "species_name": species.candidate.filter(|_| assigned).map(|s| &s.name),
        "subspecies_id": subspecies.candidate.filter(|_| assigned).map(|s| &s.id),
        "subspecies_name": subspecies.candidate.filter(|_| assigned).map(|s| &s.name),
        "lineage_id": null, "genomovar_id": null, "subspecies_reason": subspecies.reason()});
    for (key, value) in evidence
        .original_assignment()
        .as_object()
        .expect("validated assignment object")
    {
        if assignment.get(key).is_none() {
            assignment[key] = value.clone();
        }
    }
    let provenance = evidence.provenance();
    Decision {
        value: json!({"schema": "terobacter.assembly-call/1",
        "implementation": {"name": env!("CARGO_PKG_NAME"), "version": env!("CARGO_PKG_VERSION")},
        "policy": {"id": policy.id()},
        "input": {"kind": "assembly", "evidence_schema": crate::evidence::EVIDENCE_SCHEMA,
            "engine_version": provenance.engine_version, "reference_package_sha256": provenance.reference_package_sha256,
            "query_sha256": provenance.query_sha256, "evidence_sha256": evidence.source_sha256(),
            "verification": "pure_contract_checks; external_artifacts_and_origin_require_file_wrapper"},
        "assignment": assignment,
        "reference_package": package.map_or_else(|| json!({"status": "not_verified"}), VerifiedPackage::to_value),
        "species": rank_output(&species, assigned, &reason),
        "subspecies": rank_output(&subspecies, assigned, subspecies.reason()),
        "rank_evidence": {"species": species.evidence, "subspecies": subspecies.evidence},
        "blocking_reasons": reasons, "evaluation_status": if incomplete { "incomplete" } else { "completed" },
        "eligibility_state": if ineligible { "screened_ineligible" } else if incomplete { "not_assessed" } else { "screen_eligible_under_explicit_test_rule" },
        "scope_screen_state": if unexplained { "problem_unexplained_after_checks" } else if alert || method_block || !evidence.problem_warnings().is_empty() {
            "problem_flag_review_required" } else if incomplete { "incomplete" } else { "screen_completed_no_problem_flag" },
        "scope_assessment": evidence.scope(), "supplied_scope_outcome": evidence.scope()["outcome"],
        "two_parent_alert": {"triggered": alert, "qualifying_same_reference_triples": triggers, "query_AF_floor": null,
            "interpretation": "fixed developmental alert; minor or unrepresented mixtures may escape"},
        "method_diagnostics": evidence.diagnostics(),
        "external_warning_status": if evidence.warnings_supplied() { "supplied_bound_warnings" } else { "not_supplied_optional" }, "external_problem_warnings": evidence.problem_warnings(),
        "near_species_boundary": near_boundary(&rows, policy), "unmeasured_reference_count": evidence.not_returned_count(),
        "purity_established": false, "residual_sequence_explained": false}),
    }
}
