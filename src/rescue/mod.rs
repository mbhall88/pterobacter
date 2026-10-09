//! `conservative-assembly-anim-v2`: the P1-r1 ANIm species rescue, decided from saved skani
//! evidence and bound reciprocal ANIm records, plus the dry rescue plan that says which pairs a
//! decision still needs. Like [`crate::decision`], this module never touches the terminal, the
//! filesystem or a process, and it runs no alignment.
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::decision::{decide as decide_v1, Decision, Policy};
use crate::evidence::{invalid, EvidenceError, Percent, Reference, SavedEvidence, Taxon};
use crate::panel::{Panel, PanelReference};

mod records;
mod thresholds;

pub use records::AnimRecords;
pub use thresholds::Thresholds;

use records::Orientation;

pub const V2_POLICY_ID: &str = "conservative-assembly-anim-v2";
pub const CONTRACT_REVISION: &str = "P1-r1";
pub const REPORT_SCHEMA: &str = "3.0.0-conditional-assembly-report";
pub const PLAN_SCHEMA: &str = "terobacter.rescue-plan/1";
const NOT_YET_VALIDATED: &str = "not_yet_validated";
const CUSTOM_UNVALIDATED: &str = "custom_unvalidated";
const NEAREST_BELOW: &str = "nearest_below_species_ANI";
const INCOMPLETE_REASONS: [&str; 2] = ["ANIm_incomplete", "ANIm_competitor_incomplete"];

/// A competitor's parent: its species, else its lineage, else the reference alone.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ParentKey {
    Species(String),
    Lineage(String),
    Untyped(String),
}

impl ParentKey {
    fn of(species: Option<&Taxon>, lineage: Option<&String>, id: &str) -> Self {
        match (species, lineage) {
            (Some(s), _) => ParentKey::Species(s.id.clone()),
            (None, Some(l)) => ParentKey::Lineage(l.clone()),
            (None, None) => ParentKey::Untyped(id.to_owned()),
        }
    }

    fn to_json(&self) -> Value {
        match self {
            ParentKey::Species(id) => json!({"kind": "species", "concept_id": id}),
            ParentKey::Lineage(id) => json!({"kind": "lineage", "concept_id": id}),
            ParentKey::Untyped(id) => {
                json!({"kind": "untyped_reference", "concept_id": null, "reference_id": id})
            }
        }
    }
}

fn hit_parent(r: &Reference) -> ParentKey {
    ParentKey::of(r.species.as_ref(), r.lineage_id.as_ref(), &r.id)
}

fn panel_parent(r: &PanelReference) -> ParentKey {
    ParentKey::of(r.species.as_ref(), r.lineage_id.as_ref(), &r.id)
}

/// Both orientations of one bound pair; `None` is a direction with no record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pair {
    qr: Option<Orientation>,
    rq: Option<Orientation>,
}

impl Pair {
    fn present(&self) -> impl Iterator<Item = Orientation> {
        self.qr.into_iter().chain(self.rq)
    }

    fn finished(&self) -> bool {
        [self.qr, self.rq]
            .iter()
            .all(|o| o.is_some_and(|o| o.finished()))
    }

    fn strict(&self, threshold: thresholds::Exact) -> bool {
        [self.qr, self.rq]
            .iter()
            .all(|o| o.is_some_and(|o| o.strict(threshold)))
    }

    fn failed(&self) -> bool {
        self.present().any(|o| !o.finished())
    }

    /// Both orientations, plus the lower identity and the lowest of the four AFs. Each
    /// minimum is null unless both orientations measured it.
    fn to_json(self) -> Value {
        let side =
            |o: Option<Orientation>| o.map_or(json!({"status": "missing"}), Orientation::to_json);
        let least =
            |values: Option<Vec<f64>>| values.map(|v| v.into_iter().fold(f64::INFINITY, f64::min));
        let both = |f: fn(&Orientation) -> Option<Vec<f64>>| -> Option<Vec<f64>> {
            let (qr, rq) = (f(&self.qr?)?, f(&self.rq?)?);
            Some([qr, rq].concat())
        };
        json!({"QR": side(self.qr), "RQ": side(self.rq), "finished": self.finished(),
            "minimum_identity_percent": least(both(|o| o.identity_percent().map(|i| vec![i]))),
            "minimum_af_percent": least(both(|o| o.af_percents().map(Vec::from)))})
    }
}

/// How saved ANIm records were admitted to a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rule {
    /// Every provenance-bound record for this query, as P1 treats supplied evidence.
    Supplied,
    /// Only the pairs the dry plan requested under the same evidence, mask and thresholds.
    DryPlan,
}

/// ANIm records bound to one [`RescueInput`]: same query, panel reference hashes and profile.
/// Records with no query or reference binding are counted as unattributed and never bound, so
/// their pairs stay outstanding (`not_run`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundAnim {
    pairs: BTreeMap<String, Pair>,
    profile: Option<String>,
    rule: Rule,
    excluded: Vec<(String, &'static str)>,
    other_query: usize,
    unattributed: usize,
}

impl BoundAnim {
    /// No ANIm at all, as for a first dry plan.
    pub fn none() -> Self {
        BoundAnim {
            pairs: BTreeMap::new(),
            profile: None,
            rule: Rule::Supplied,
            excluded: vec![],
            other_query: 0,
            unattributed: 0,
        }
    }

    /// Whether both orientations of the pair were measured, aligned or not.
    fn finished(&self, id: &str) -> bool {
        self.pairs.get(id).is_some_and(Pair::finished)
    }

    fn to_json(&self) -> Value {
        json!({"binding": match self.rule { Rule::Supplied => "supplied_records", Rule::DryPlan => "dry_plan_requested_pairs" },
            "source": "supplied_saved_records", "method_profile": self.profile,
            "bound_pairs": self.pairs.keys().collect::<Vec<_>>(),
            "excluded": self.excluded.iter().map(|(id, why)| json!({"reference_id": id, "reason": why})).collect::<Vec<_>>(),
            "other_query_records": self.other_query, "unattributed_records": self.unattributed})
    }
}

/// Validated v2 inputs for one query under one setting: masked saved evidence, the masked
/// primary panel and the thresholds.
#[derive(Debug, Clone)]
pub struct RescueInput {
    evidence: SavedEvidence,
    panel: Panel,
    full_panel: Panel,
    mask: BTreeSet<String>,
    thresholds: Thresholds,
}

/// The v1 skani rules evaluated on the same evidence, read back from the v1 decision.
struct SkaniOutcome {
    assigned: bool,
    species: (Value, Value),
    subspecies: (Value, Value),
    reasons: Vec<String>,
    subspecies_reason: Value,
    assignment_reason: Value,
}

impl SkaniOutcome {
    fn of(decision: &Decision) -> Self {
        let v = decision.to_value();
        let a = &v["assignment"];
        SkaniOutcome {
            assigned: a["status"] == "assigned",
            species: (a["species_id"].clone(), a["species_name"].clone()),
            subspecies: (a["subspecies_id"].clone(), a["subspecies_name"].clone()),
            reasons: v["blocking_reasons"]
                .as_array()
                .expect("v1 reasons")
                .iter()
                .map(|r| r.as_str().expect("reason text").to_owned())
                .collect(),
            subspecies_reason: a["subspecies_reason"].clone(),
            assignment_reason: a["reason"].clone(),
        }
    }

    fn to_json(&self, policy: &str) -> Value {
        json!({"policy_id": policy, "status": if self.assigned { "assigned" } else { "unresolved" },
            "species_id": self.species.0, "species_name": self.species.1,
            "subspecies_id": self.subspecies.0, "subspecies_name": self.subspecies.1,
            "reasons": self.reasons, "subspecies_reason": self.subspecies_reason,
            "assignment_reason": self.assignment_reason})
    }
}

/// Where the rescue stands and which pairs it needs next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    NotTriggered,
    Blocked,
    NoSkaniWitness,
    Witness,
    Panel,
}

impl Stage {
    fn as_str(self) -> &'static str {
        match self {
            Stage::NotTriggered => "not_triggered",
            Stage::Blocked => "blocked_before_anim",
            Stage::NoSkaniWitness => "no_skani_witness",
            Stage::Witness => "witness",
            Stage::Panel => "competitor_panel",
        }
    }
}

/// The evaluation's trigger states. Under custom thresholds, the 96 in a name stands for T.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TriggerState {
    NoEstimate,
    AtOrAboveSpecies,
    BelowBand,
    CrossParentTie,
    UnnamedNearest,
    NoMeasuredContrast,
    NoSkaniWitness,
    WitnessCandidate,
}

impl TriggerState {
    fn as_str(self) -> &'static str {
        match self {
            TriggerState::NoEstimate => "no_estimate",
            TriggerState::AtOrAboveSpecies => "at_or_above_96_v1_route",
            TriggerState::BelowBand => "below_band_v1_route",
            TriggerState::CrossParentTie => "triggered_cross_parent_tie",
            TriggerState::UnnamedNearest => "triggered_unnamed_nearest",
            TriggerState::NoMeasuredContrast => "triggered_no_measured_contrast",
            TriggerState::NoSkaniWitness => "triggered_no_skani_witness",
            TriggerState::WitnessCandidate => "triggered_with_witness_candidate",
        }
    }
}

/// Everything P1 derives from the evidence before the final call.
struct Analysis<'a> {
    nearest: Option<&'a Reference>,
    band: bool,
    state: TriggerState,
    candidates: Vec<&'a Reference>,
    witnesses: Vec<&'a Reference>,
    blocking: Vec<String>,
    challenges: Vec<Value>,
    method_vetoes: BTreeSet<&'static str>,
    diagnostics: Vec<Value>,
    skani: SkaniOutcome,
    stage: Stage,
}

impl Analysis<'_> {
    fn challenge_ids(&self) -> Vec<Value> {
        self.challenges
            .iter()
            .map(|c| c["reference_id"].clone())
            .collect()
    }
}

fn percent(p: Percent) -> Value {
    json!(p.as_f64())
}

fn skani_triple(r: &Reference) -> Value {
    json!({"ani_percent": percent(r.ani), "af_query_percent": percent(r.af_query),
        "af_reference_percent": percent(r.af_reference)})
}

impl RescueInput {
    /// Bind the panel to the evidence and apply the mask to both. Every masked reference must
    /// be in the panel; it leaves the skani rows, the panel counts and the ANIm competitor set.
    pub fn new(
        evidence: SavedEvidence,
        panel: Panel,
        mask: &[String],
        thresholds: Thresholds,
    ) -> Result<Self, EvidenceError> {
        if panel.evidence_sha256() != evidence.source_sha256() {
            return invalid("the primary inventory was checked against different evidence");
        }
        let mut set = BTreeSet::new();
        for id in mask {
            if panel.get(id).is_none() {
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
        Ok(RescueInput {
            evidence: evidence.without_references(&set, unreturned),
            panel: panel.without(&set),
            mask: set.iter().map(|id| (*id).to_owned()).collect(),
            full_panel: panel,
            thresholds,
        })
    }

    /// Bind every record for this query whose provenance matches: query hash and length,
    /// panel reference hash, and `profile`. Records for other queries are skipped and counted;
    /// masked references are excluded. Mismatched provenance for this query is an error.
    pub fn bind_supplied(
        &self,
        records: &AnimRecords,
        profile: &str,
    ) -> Result<BoundAnim, EvidenceError> {
        let query = &self.evidence.provenance().query_sha256;
        let mut bound = BoundAnim {
            profile: Some(profile.to_owned()),
            ..BoundAnim::none()
        };
        let mut query_bases = None;
        for record in &records.records {
            let (Some(q), Some(id)) = (&record.query_sha256, &record.reference_id) else {
                bound.unattributed += 1;
                continue;
            };
            if q != query {
                bound.other_query += 1;
                continue;
            }
            if record.profile != profile {
                return invalid(format!(
                    "{id}: ANIm method profile {} is not the expected {profile}",
                    record.profile
                ));
            }
            if *query_bases.get_or_insert(record.query_bases) != record.query_bases {
                return invalid("ANIm records disagree on the query length");
            }
            let Some(reference) = self.full_panel.get(id) else {
                return invalid(format!("{id}: ANIm reference is not in the primary panel"));
            };
            if record.reference_sha256.as_deref() != Some(reference.source_sha256.as_str()) {
                return invalid(format!(
                    "{id}: ANIm reference hash does not match the panel"
                ));
            }
            if self.mask.contains(id) {
                bound.excluded.push((id.clone(), "masked_reference"));
                continue;
            }
            let pair = Pair {
                qr: record.qr,
                rq: record.rq,
            };
            match bound.pairs.get(id) {
                Some(existing) if *existing != pair => {
                    return invalid(format!("{id}: conflicting ANIm records for one pair"))
                }
                Some(_) => {}
                None => {
                    bound.pairs.insert(id.clone(), pair);
                }
            }
        }
        Ok(bound)
    }

    /// Bind only the pairs the dry plan requests under these settings. Witness records decide
    /// whether the competitor panel is requested; any other record for this query is excluded.
    pub fn bind_to_plan(
        &self,
        records: &AnimRecords,
        profile: &str,
    ) -> Result<BoundAnim, EvidenceError> {
        let mut bound = self.bind_supplied(records, profile)?;
        bound.rule = Rule::DryPlan;
        let candidates: BTreeSet<String> = self
            .analyse(&BoundAnim::none())
            .candidates
            .iter()
            .map(|r| r.id.clone())
            .collect();
        let witnesses_only = BoundAnim {
            pairs: bound
                .pairs
                .iter()
                .filter(|(id, _)| candidates.contains(*id))
                .map(|(id, p)| (id.clone(), *p))
                .collect(),
            ..bound.clone()
        };
        let requested = self.requested(&self.analyse(&witnesses_only));
        let (kept, dropped): (BTreeMap<_, _>, BTreeMap<_, _>) = bound
            .pairs
            .into_iter()
            .partition(|(id, _)| requested.contains(id));
        bound.pairs = kept;
        bound.excluded.extend(
            dropped
                .into_keys()
                .map(|id| (id, "not_requested_by_dry_plan")),
        );
        Ok(bound)
    }

    /// The pairs a decision needs at this stage: witness candidates in witness order, then,
    /// for the competitor panel, every other panel reference by ID.
    fn requested(&self, analysis: &Analysis<'_>) -> Vec<String> {
        let mut ids: Vec<String> = analysis.candidates.iter().map(|r| r.id.clone()).collect();
        match analysis.stage {
            Stage::Witness => {}
            Stage::Panel => {
                let mut rest: Vec<String> = self
                    .panel
                    .references()
                    .iter()
                    .map(|r| r.id.clone())
                    .filter(|id| !ids.contains(id))
                    .collect();
                rest.sort();
                ids.extend(rest);
            }
            _ => ids.clear(),
        }
        ids
    }

    fn skani_policy(&self) -> Policy {
        let t = &self.thresholds;
        Policy::skani_route(
            V2_POLICY_ID,
            t.species().ceil_hundredths(),
            t.subspecies().ceil_hundredths(),
            t.conflict().ceil_hundredths(),
        )
    }

    fn analyse<'a>(&'a self, anim: &BoundAnim) -> Analysis<'a> {
        let t = &self.thresholds;
        let species_ani = t.species().ceil_hundredths();
        let lower = t.trigger_lower().ceil_hundredths();
        let conflict = t.conflict().ceil_hundredths();
        let af = Percent::from_whole(80);
        let hits = self.evidence.references();
        let nearest = hits.first();
        let band = nearest.is_some_and(|n| lower <= n.ani && n.ani < species_ani);
        let nearest_parent = nearest.map(hit_parent);
        let skani = SkaniOutcome::of(&decide_v1(&self.evidence, &self.skani_policy()));

        let candidates: Vec<&Reference> = hits
            .iter()
            .filter(|r| {
                Some(hit_parent(r)) == nearest_parent
                    && r.species.is_some()
                    && lower <= r.ani
                    && r.ani < species_ani
                    && r.min_af() >= af
            })
            .collect();
        let state = match nearest {
            None => TriggerState::NoEstimate,
            Some(n) if n.ani >= species_ani => TriggerState::AtOrAboveSpecies,
            Some(n) if n.ani < lower => TriggerState::BelowBand,
            Some(n) => {
                let tied: BTreeSet<_> = hits
                    .iter()
                    .take_while(|r| r.ani == n.ani)
                    .map(hit_parent)
                    .collect();
                if tied.len() > 1 {
                    TriggerState::CrossParentTie
                } else if n.species.is_none() {
                    TriggerState::UnnamedNearest
                } else if hits.iter().all(|r| Some(hit_parent(r)) == nearest_parent) {
                    TriggerState::NoMeasuredContrast
                } else if candidates.is_empty() {
                    TriggerState::NoSkaniWitness
                } else {
                    TriggerState::WitnessCandidate
                }
            }
        };

        // The old method gate, kept visible, with only the same-parent ANI crossing exempt.
        let skani_gate = |r: &Reference| r.ani >= species_ani && r.min_af() >= af;
        let mut method_vetoes = BTreeSet::new();
        let mut diagnostics = Vec::new();
        for hit in hits {
            let Some(pair) = anim.pairs.get(&hit.id) else {
                continue;
            };
            let present: Vec<_> = pair.present().collect();
            if present.iter().any(|o| !o.finished()) || present.len() != 2 {
                method_vetoes.insert("ANIm_incomplete");
            }
            let skani_passes = skani_gate(hit);
            let disagreement = present
                .iter()
                .filter(|o| o.finished())
                .any(|o| o.gate(t.species()) != skani_passes);
            let crossing =
                band && candidates.iter().any(|c| c.id == hit.id) && pair.strict(t.species());
            if disagreement && !crossing {
                method_vetoes.insert("method_gate_disagreement");
            }
            let gate =
                |o: Option<Orientation>| o.filter(|o| o.finished()).map(|o| o.gate(t.species()));
            diagnostics.push(json!({"reference_id": hit.id, "skani": skani_triple(hit),
                "skani_meets_species_gate": skani_passes,
                "anim_meets_species_gate": {"QR": gate(pair.qr), "RQ": gate(pair.rq)},
                "gate_disagreement": disagreement,
                "expected_ANI_method_crossing": disagreement && crossing,
                "vetoes": disagreement && !crossing}));
        }

        // Support for any other parent, from skani or either ANIm orientation, across the panel.
        let mut challenges = Vec::new();
        if band {
            for r in self.panel.references() {
                if Some(panel_parent(r)) == nearest_parent {
                    continue;
                }
                let hit = hits.iter().find(|h| h.id == r.id);
                let by_skani = hit.is_some_and(|h| h.ani >= conflict && h.af_reference >= af);
                let pair = anim.pairs.get(&r.id);
                let by_anim: Vec<_> = pair
                    .iter()
                    .flat_map(|p| [("QR", p.qr), ("RQ", p.rq)])
                    .filter(|(_, o)| o.is_some_and(|o| o.challenges(t.conflict())))
                    .map(|(name, _)| name)
                    .collect();
                if by_skani || !by_anim.is_empty() {
                    challenges.push(
                        json!({"reference_id": r.id, "parent": panel_parent(r).to_json(),
                        "skani": hit.map(skani_triple), "skani_challenges": by_skani,
                        "anim_challenging_orientations": by_anim}),
                    );
                }
            }
            challenges.sort_by(|a, b| a["reference_id"].as_str().cmp(&b["reference_id"].as_str()));
        }

        let blocking: Vec<String> = skani
            .reasons
            .iter()
            .filter(|r| *r != NEAREST_BELOW)
            .cloned()
            .collect();
        let witnesses: Vec<&Reference> = candidates
            .iter()
            .copied()
            .filter(|r| anim.pairs.get(&r.id).is_some_and(|p| p.strict(t.species())))
            .collect();
        let stage = if !band {
            Stage::NotTriggered
        } else if !blocking.is_empty() {
            Stage::Blocked
        } else if candidates.is_empty() {
            Stage::NoSkaniWitness
        } else if witnesses.is_empty() {
            Stage::Witness
        } else {
            Stage::Panel
        };
        Analysis {
            nearest,
            band,
            state,
            candidates,
            witnesses,
            blocking,
            challenges,
            method_vetoes,
            diagnostics,
            skani,
            stage,
        }
    }

    /// Pair counts. Each attempted pair is completed, failed (an orientation failed) or
    /// missing an orientation record; a required pair with no record is not run.
    fn counts(&self, requested: &[String], anim: &BoundAnim) -> Value {
        let attempted: Vec<&Pair> = requested
            .iter()
            .filter_map(|id| anim.pairs.get(id))
            .collect();
        let count = |f: fn(&Pair) -> bool| attempted.iter().filter(|p| f(p)).count();
        let no_alignment =
            count(|p| p.finished() && p.present().any(|o| o == Orientation::NoAlignment));
        json!({"unit": "reciprocal_pair", "required": requested.len(), "attempted": attempted.len(),
            "completed": count(Pair::finished), "no_alignment": no_alignment,
            "failed": count(Pair::failed),
            "missing_orientation": count(|p| !p.finished() && !p.failed()),
            "not_run": requested.len() - attempted.len(),
            "supplied_outside_plan": anim.pairs.keys().filter(|id| !requested.contains(*id)).count()})
    }

    fn policy_json(&self) -> Value {
        let custom = self.thresholds.is_custom();
        json!({"policy_id": V2_POLICY_ID, "contract_revision": CONTRACT_REVISION,
            "policy_mode": if custom { "custom" } else { "default_candidate" },
            "validation_status": if custom { CUSTOM_UNVALIDATED } else { NOT_YET_VALIDATED },
            "thresholds": self.thresholds.to_json()})
    }

    fn input_json(&self, anim: &BoundAnim) -> Value {
        let p = self.evidence.provenance();
        json!({"kind": "assembly", "evidence_schema": crate::evidence::EVIDENCE_SCHEMA,
            "engine_version": p.engine_version, "reference_package_sha256": p.reference_package_sha256,
            "query_sha256": p.query_sha256, "evidence_sha256": self.evidence.source_sha256(),
            "primary_inventory_sha256": self.full_panel.source_sha256(),
            "mask": {"excluded_reference_ids": self.mask, "panel_reference_count": self.full_panel.references().len(),
                "assessed_panel_reference_count": self.panel.references().len()},
            "anim": anim.to_json(),
            "verification": "pure_contract_checks; external_artifacts_and_origin_require_file_wrapper"})
    }

    fn trigger_json(&self, a: &Analysis<'_>) -> Value {
        json!({"trigger": a.band, "trigger_state": a.state.as_str(),
        "max_skani_ani_percent": a.nearest.map(|n| percent(n.ani)),
        "nearest_parent": a.nearest.filter(|_| a.band).map(|n| {
            let mut parent = hit_parent(n).to_json();
            parent["name"] = json!(n.species.as_ref().map(|s| &s.name));
            parent
        })})
    }

    /// The dry rescue plan: trigger, nearest parent, witnesses and the pairs still required.
    /// It reads saved evidence only and executes no alignment.
    pub fn plan(&self, anim: &BoundAnim) -> RescuePlan {
        let a = self.analyse(anim);
        let requested = self.requested(&a);
        let source = |id: &str| self.panel.get(id).map(|r| r.source_sha256.clone());
        let pair_stage = if a.stage == Stage::Panel {
            "competitor_panel"
        } else {
            "witness"
        };
        let pairs: Vec<Value> = requested.iter().map(|id| {
            let status = match anim.pairs.get(id) {
                None => "outstanding",
                Some(p) if p.finished() => "measured",
                Some(p) if p.failed() => "failed",
                Some(_) => "missing_orientation",
            };
            json!({"reference_id": id, "reference_sha256": source(id),
                "stage": if a.candidates.iter().any(|c| &c.id == id) { "witness" } else { pair_stage },
                "status": status})
        }).collect();
        let outstanding = |stage: Stage| -> Vec<&String> {
            if a.stage != stage {
                return vec![];
            }
            requested.iter().filter(|id| !anim.finished(id)).collect()
        };
        let witnesses = if a.state == TriggerState::WitnessCandidate {
            a.candidates.clone()
        } else {
            vec![]
        };
        let mut decisive: BTreeSet<&str> = a
            .method_vetoes
            .iter()
            .copied()
            .filter(|r| !INCOMPLETE_REASONS.contains(r))
            .collect();
        if !a.challenges.is_empty() {
            decisive.insert("foreign_parent_challenge");
        }
        let mut value = json!({"schema": PLAN_SCHEMA,
            "implementation": {"name": env!("CARGO_PKG_NAME"), "version": env!("CARGO_PKG_VERSION")},
            "anim_executed": false, "input": self.input_json(anim),
            "stage": a.stage.as_str(),
            "witness_candidates": witnesses.iter().map(|r| json!({"reference_id": r.id,
                "reference_sha256": source(&r.id), "skani": skani_triple(r)})).collect::<Vec<_>>(),
            "passing_witness": a.witnesses.first().map(|r| &r.id),
            "blocking_reasons": if a.stage == Stage::NoSkaniWitness { vec!["skani_coverage_not_rescuable".to_owned()] } else { a.blocking.clone() },
            "requested_pairs": pairs,
            "outstanding_witnesses": outstanding(Stage::Witness),
            "outstanding_competitors": outstanding(Stage::Panel),
            "known_challenges": a.challenge_ids(),
            "decisive_vetoes": decisive,
            "counts": self.counts(&requested, anim)});
        merge(&mut value, self.policy_json());
        merge(&mut value, self.trigger_json(&a));
        RescuePlan { value }
    }

    /// Decide v2. Outside the trigger band the v1 skani rules apply, plus the old method veto
    /// for any supplied ANIm. Inside it, P1's ordered rescue applies.
    pub fn decide(&self, anim: &BoundAnim) -> Decision {
        let a = self.analyse(anim);
        let t = &self.thresholds;
        let vetoes: BTreeSet<String> = a.method_vetoes.iter().map(|r| (*r).to_owned()).collect();
        let mut competitor_evaluation = "not_required";
        let mut rescued = None;
        let reasons: BTreeSet<String> = if !a.band {
            a.skani.reasons.iter().cloned().chain(vetoes).collect()
        } else {
            let mut reasons: BTreeSet<String> = vetoes;
            match a.stage {
                Stage::Blocked => {
                    reasons.extend(a.blocking.iter().cloned());
                    competitor_evaluation = "not_run_already_blocked";
                }
                Stage::NoSkaniWitness => {
                    reasons.insert("skani_coverage_not_rescuable".into());
                }
                Stage::Witness => {
                    let incomplete = a.candidates.iter().any(|r| !anim.finished(&r.id));
                    reasons.insert(
                        if incomplete {
                            "ANIm_incomplete"
                        } else {
                            "ANIm_no_strict_witness"
                        }
                        .into(),
                    );
                    competitor_evaluation = "not_required_no_witness";
                }
                Stage::Panel => {
                    let missing = self
                        .panel
                        .references()
                        .iter()
                        .any(|r| !anim.finished(&r.id));
                    competitor_evaluation = if missing { "incomplete" } else { "complete" };
                    if missing {
                        reasons.insert("ANIm_competitor_incomplete".into());
                    }
                    if !a.challenges.is_empty() {
                        reasons.insert("foreign_parent_challenge".into());
                    }
                    if reasons.is_empty() {
                        rescued = a.witnesses.first().copied();
                    }
                }
                Stage::NotTriggered => unreachable!("in band"),
            }
            reasons
        };
        let skani_call = !a.band && reasons.is_empty() && a.skani.assigned;
        let assigned = skani_call || rescued.is_some();
        let (species, subspecies, subspecies_reason) = if let Some(w) = rescued {
            let s = w.species.as_ref().expect("witness in a named species");
            (
                (json!(s.id), json!(s.name)),
                (Value::Null, Value::Null),
                json!("ANIm_species_rescue_does_not_establish_subspecies"),
            )
        } else if skani_call {
            (
                a.skani.species.clone(),
                a.skani.subspecies.clone(),
                a.skani.subspecies_reason.clone(),
            )
        } else {
            (
                (Value::Null, Value::Null),
                (Value::Null, Value::Null),
                a.skani.subspecies_reason.clone(),
            )
        };
        let rescue_status = if !a.band {
            "not_triggered"
        } else if rescued.is_some() {
            "rescued"
        } else if reasons
            .iter()
            .all(|r| INCOMPLETE_REASONS.contains(&r.as_str()))
        {
            "incomplete"
        } else {
            "withheld"
        };
        let incomplete = reasons.iter().any(|r| {
            INCOMPLETE_REASONS.contains(&r.as_str()) || r == "species_competitor_unmeasured"
        });
        let requested = self.requested(&a);
        let legacy = SkaniOutcome::of(&decide_v1(&self.evidence, &Policy::v1()));
        let witness_assessment: Vec<Value> = a.candidates.iter().map(|r| {
            let pair = anim.pairs.get(&r.id);
            json!({"reference_id": r.id, "skani": skani_triple(r), "anim": pair.map(|p| p.to_json()),
                "strict_pass": pair.is_some_and(|p| p.strict(t.species())),
                "finished": anim.finished(&r.id)})
        }).collect();
        let mut value = json!({"schema": REPORT_SCHEMA,
            "implementation": {"name": env!("CARGO_PKG_NAME"), "version": env!("CARGO_PKG_VERSION")},
            "input": self.input_json(anim),
            "status": if assigned { "assigned" } else { "unresolved" },
            "species_id": species.0, "species_name": species.1,
            "subspecies_id": subspecies.0, "subspecies_name": subspecies.1,
            "subspecies_reason": subspecies_reason,
            "call_basis": if rescued.is_some() { json!("ANIm_assisted") } else if skani_call { json!("skani") } else { Value::Null },
            "reasons": reasons,
            "rescue_status": rescue_status,
            "competitor_evaluation": competitor_evaluation,
            "evaluation_status": if incomplete { "incomplete" } else { "complete" },
            "counts": self.counts(&requested, anim),
            "witness": rescued.map(|w| json!({"reference_id": w.id, "skani": skani_triple(w),
                "anim": anim.pairs.get(&w.id).map(|p| p.to_json())})),
            "witness_assessment": witness_assessment,
            "challenges": a.challenge_ids(),
            "challenge_evidence": a.challenges,
            "method_diagnostics": a.diagnostics,
            "top_references": self.top_references(anim),
            "panel_summary": self.panel_summary(anim, &a),
            "legacy_v1_result": legacy.to_json(crate::decision::V1_POLICY_ID),
            "scope_assessment": self.evidence.scope(),
            "external_problem_warnings": self.evidence.problem_warnings(),
            "purity_established": false});
        merge(&mut value, self.policy_json());
        merge(&mut value, self.trigger_json(&a));
        Decision::from_value(value)
    }

    /// The five highest skani references, plus every exact tie at fifth place.
    fn shown(&self) -> &[Reference] {
        let hits = self.evidence.references();
        let cut = hits.get(4).map(|r| r.ani);
        let n = hits
            .iter()
            .take_while(|r| cut.is_none_or(|c| r.ani >= c))
            .count();
        &hits[..n]
    }

    /// The [`shown`](Self::shown) references. Ties are ordered by reference ID for display only.
    fn top_references(&self, anim: &BoundAnim) -> Vec<Value> {
        let hits = self.shown();
        let mut group = 0;
        hits.iter()
            .enumerate()
            .map(|(i, r)| {
                if i > 0 && hits[i - 1].ani != r.ani {
                    group += 1;
                }
                let panel = self.panel.get(&r.id).expect("returned rows are in the panel");
                json!({"reference_id": r.id, "tie_group": group,
                    "species_id": r.species.as_ref().map(|s| &s.id), "species_name": r.species.as_ref().map(|s| &s.name),
                    "subspecies_id": r.subspecies.as_ref().map(|s| &s.id), "lineage_id": r.lineage_id,
                    "reference_role": r.reference_role, "parent": hit_parent(r).to_json(),
                    "reference_sha256": panel.source_sha256, "skani": skani_triple(r),
                    "anim": anim.pairs.get(&r.id).map(|p| p.to_json())})
            })
            .collect()
    }

    /// Every parent in the assessed panel, with its best skani evidence and any challenge,
    /// whether or not it reaches the displayed top references.
    fn panel_summary(&self, anim: &BoundAnim, a: &Analysis<'_>) -> Vec<Value> {
        let hits = self.evidence.references();
        let shown: BTreeSet<&str> = self.shown().iter().map(|r| r.id.as_str()).collect();
        let challenged: BTreeSet<&str> = a
            .challenges
            .iter()
            .filter_map(|c| c["reference_id"].as_str())
            .collect();
        let mut groups: BTreeMap<ParentKey, Vec<&PanelReference>> = BTreeMap::new();
        for r in self.panel.references() {
            groups.entry(panel_parent(r)).or_default().push(r);
        }
        let mut rows: Vec<(Option<Percent>, Value)> = groups.into_iter().map(|(parent, members)| {
            let returned: Vec<&Reference> = hits.iter().filter(|h| members.iter().any(|m| m.id == h.id)).collect();
            let best = returned.first().map(|h| h.ani);
            let unmeasured: Vec<&String> = members.iter().map(|m| &m.id)
                .filter(|id| !anim.finished(id)).collect();
            (best, json!({"parent": parent.to_json(), "panel_reference_count": members.len(),
                "returned_reference_count": returned.len(),
                "best_skani_ani_percent": best.map(percent),
                "best_skani_reference_ids": returned.iter().take_while(|h| Some(h.ani) == best).map(|h| &h.id).collect::<Vec<_>>(),
                "in_top_references": returned.iter().any(|h| shown.contains(h.id.as_str())),
                "challenging_reference_ids": members.iter().map(|m| m.id.as_str()).filter(|id| challenged.contains(id)).collect::<Vec<_>>(),
                "anim_unmeasured_reference_count": unmeasured.len()}))
        }).collect();
        rows.sort_by(|(x, _), (y, _)| y.cmp(x));
        rows.into_iter().map(|(_, v)| v).collect()
    }
}

/// A dry rescue plan; see [`RescueInput::plan`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RescuePlan {
    value: Value,
}

impl RescuePlan {
    pub fn to_value(&self) -> &Value {
        &self.value
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.value).expect("a JSON value always serialises")
    }
}

fn merge(target: &mut Value, extra: Value) {
    for (k, v) in extra.as_object().expect("object").clone() {
        target[k] = v;
    }
}
