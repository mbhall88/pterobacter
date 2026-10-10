//! Conditional genomovar reporting under the accepted P3a contract (`p3a-genomovar-contract-r1`).
//!
//! A genomovar is an informal chromosome ANI group within subspecies `ECTX-SS007`, frozen as 25
//! groups of 173 historical chromosome members. A query is matched to a group only when it has
//! the actually assigned parent, reported skani ANI of at least 99.50% and both aligned fractions
//! of at least 80% to every member of exactly one group, and a measured below-threshold member
//! excluding every other group. Nothing here reclusters, changes a membership or learns from a
//! query. Like [`crate::decision`], this module is pure: the front end supplies the measured rows
//! and the context its adapter verified.
use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::evidence::Percent;

pub const POLICY_ID: &str = "p3a-genomovar-contract-r1";
pub const DEFINITION_VERSION: &str = "2026-10-02-v1";
pub const PARENT_SPECIES_ID: &str = "ECTX-SP025";
pub const PARENT_SUBSPECIES_ID: &str = "ECTX-SS007";
/// The pinned genomovar search: skani 0.3.2, compression 125, marker compression 1000, learned
/// ANI, screen 80, reporting floor 0 and no result cap.
pub const ENGINE: &str = "skani";
pub const ENGINE_VERSION: &str = "0.3.2";
pub const SEARCH_PARAMETERS: &str = "c125_m1000_learnedANI_s80_minAF0_uncapped";
pub const QUERY_SCOPE: &str = "whole_assembly_as_supplied";
pub const REFERENCE_SCOPE: &str = "historical_chromosome_record_set";
pub const COMPONENT_ID: &str = "genomovar_chromosome";

const ANI_MINIMUM: Percent = Percent::from_hundredths(9950);
const AF_MINIMUM: Percent = Percent::from_whole(80);

#[derive(Debug, thiserror::Error)]
#[error("genomovar evidence rejected: {0}")]
pub struct GenomovarError(String);

fn reject<T>(message: impl Into<String>) -> Result<T, GenomovarError> {
    Err(GenomovarError(message.into()))
}

/// The groups of one definition version, each with its whole required member set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    version: String,
    groups: BTreeMap<String, Vec<String>>,
}

impl Definition {
    /// Groups keyed by genomovar ID. A member may belong to one group only. An empty group is
    /// allowed for a development self-exclusion diagnostic; it can never be matched.
    pub fn new(
        version: &str,
        groups: BTreeMap<String, Vec<String>>,
    ) -> Result<Self, GenomovarError> {
        if groups.is_empty() {
            return reject("explicit groups required");
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut sorted = BTreeMap::new();
        for (id, members) in groups {
            if id.is_empty() || members.iter().any(String::is_empty) {
                return reject("invalid group or member identifier");
            }
            for member in &members {
                if !seen.insert(member.clone()) {
                    return reject(format!("{member}: duplicate definition member"));
                }
            }
            let mut members = members;
            members.sort();
            sorted.insert(id, members);
        }
        Ok(Definition {
            version: version.to_owned(),
            groups: sorted,
        })
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn groups(&self) -> &BTreeMap<String, Vec<String>> {
        &self.groups
    }

    pub fn member_count(&self) -> usize {
        self.groups.values().map(Vec::len).sum()
    }

    fn contains(&self, member: &str) -> bool {
        self.groups
            .values()
            .any(|ms| ms.iter().any(|m| m == member))
    }

    /// SHA-256 of `group<TAB>member<LF>` lines in group then member order.
    pub fn membership_sha256(&self) -> String {
        let mut hasher = Sha256::new();
        for (group, members) in &self.groups {
            for member in members {
                hasher.update(format!("{group}\t{member}\n"));
            }
        }
        format!("{:x}", hasher.finalize())
    }
}

/// One reported skani comparison between the query and a definition member.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberRow {
    pub reference_id: String,
    #[serde(rename = "ani_percent")]
    pub ani: Percent,
    #[serde(rename = "aligned_fraction_query_percent")]
    pub af_query: Percent,
    #[serde(rename = "aligned_fraction_reference_percent")]
    pub af_reference: Percent,
}

/// The parent call the genomovar depends on, as read from a final decision.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parent {
    pub policy_id: String,
    pub status: String,
    pub species_id: Option<String>,
    pub subspecies_id: Option<String>,
    /// The adapter checked the decision's provenance and, for v2, the bridge to v1.
    pub verified: bool,
    pub query_sha256: String,
    pub package_sha256: String,
    pub blocking_reasons: Vec<String>,
    pub scope_outcome: String,
}

/// What the front end's adapter verified about the parent and the genomovar search.
/// The default context is empty and fails closed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Context {
    pub parent: Parent,
    pub input_kind: String,
    pub query_scope: String,
    pub reference_scope: String,
    pub engine: String,
    pub engine_version: String,
    pub parameters: String,
    pub search_status: String,
    /// Every row was bound to a definition member of the verified package component.
    pub adapter_verified: bool,
    pub query_sha256: String,
    pub package_sha256: String,
    /// A declared development leave-one-out diagnostic; never a deployment call.
    pub diagnostic_exclusion: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Compatible,
    Incompatible,
    Incomplete,
    Unassessable,
}

impl State {
    fn as_str(self) -> &'static str {
        match self {
            State::Compatible => "compatible",
            State::Incompatible => "incompatible",
            State::Incomplete => "incomplete",
            State::Unassessable => "unassessable",
        }
    }
}

#[derive(Debug, Clone)]
struct Cell {
    group: String,
    state: State,
    required: usize,
    observed: Vec<String>,
    missing: Vec<String>,
    below_threshold: Vec<String>,
    below_member_af: Vec<String>,
    minimum: Option<(Percent, Percent, Percent)>,
}

/// The result of [`assess`].
#[derive(Debug, Clone)]
pub struct Assessment {
    version: String,
    membership_sha256: String,
    member_count: usize,
    assigned: Option<String>,
    compatible: Vec<String>,
    incomplete: Vec<String>,
    reasons: Vec<&'static str>,
    cells: Vec<Cell>,
    warnings: Vec<&'static str>,
}

impl Assessment {
    pub fn assigned_genomovar_id(&self) -> Option<&str> {
        self.assigned.as_deref()
    }

    pub fn to_value(&self) -> Value {
        let pct = |p: Percent| serde_json::to_value(p).expect("percent");
        let groups: Vec<Value> = self
            .cells
            .iter()
            .map(|c| {
                json!({"group": c.group, "state": c.state.as_str(), "required": c.required,
                "observed": c.observed.len(), "missing": c.missing,
                "below_threshold": c.below_threshold, "below_member_af": c.below_member_af,
                "minimum_ani_percent": c.minimum.map(|m| pct(m.0)),
                "minimum_af_query_percent": c.minimum.map(|m| pct(m.1)),
                "minimum_af_reference_percent": c.minimum.map(|m| pct(m.2))})
            })
            .collect();
        json!({"policy_id": POLICY_ID,
            "status": if self.assigned.is_some() { "assigned" } else { "withheld" },
            "genomovar_id": self.assigned,
            "reasons": self.reasons,
            "compatible_groups": self.compatible,
            "incomplete_groups": self.incomplete,
            "groups": groups,
            "warnings": self.warnings,
            "definition": {"version": self.version, "group_count": self.cells.len(),
                "member_count": self.member_count, "membership_sha256": self.membership_sha256},
            "rule": {"ani_minimum_percent": pct(ANI_MINIMUM), "every_member": true,
                "both_af_minimum_percent": pct(AF_MINIMUM),
                "rival_exclusion": "measured_below_threshold_member"}})
    }
}

/// Apply the all-member predicate, then the parent, scope, method and coverage gates. Every
/// failing gate is reported; none hides another.
pub fn assess(
    definition: &Definition,
    rows: &[MemberRow],
    context: &Context,
) -> Result<Assessment, GenomovarError> {
    let mut hits: BTreeMap<&str, &MemberRow> = BTreeMap::new();
    for row in rows {
        if !definition.contains(&row.reference_id) || hits.contains_key(row.reference_id.as_str()) {
            return reject(format!(
                "{}: unexpected or duplicate member evidence",
                row.reference_id
            ));
        }
        if row.af_query == Percent::from_whole(0) || row.af_reference == Percent::from_whole(0) {
            return reject(format!(
                "{}: a returned ANI needs positive aligned fractions",
                row.reference_id
            ));
        }
        hits.insert(&row.reference_id, row);
    }
    let cells: Vec<Cell> = definition
        .groups
        .iter()
        .map(|(group, members)| {
            let (observed, missing): (Vec<&String>, Vec<&String>) =
                members.iter().partition(|m| hits.contains_key(m.as_str()));
            let found: Vec<&MemberRow> = observed.iter().map(|m| hits[m.as_str()]).collect();
            let below_threshold = found
                .iter()
                .filter(|r| r.ani < ANI_MINIMUM)
                .map(|r| r.reference_id.clone())
                .collect::<Vec<_>>();
            let below_member_af = found
                .iter()
                .filter(|r| r.af_query.min(r.af_reference) < AF_MINIMUM)
                .map(|r| r.reference_id.clone())
                .collect();
            let minimum = (!found.is_empty()).then(|| {
                (
                    found.iter().map(|r| r.ani).min().expect("observed"),
                    found.iter().map(|r| r.af_query).min().expect("observed"),
                    found
                        .iter()
                        .map(|r| r.af_reference)
                        .min()
                        .expect("observed"),
                )
            });
            let state = if members.is_empty() {
                State::Unassessable
            } else if !below_threshold.is_empty() {
                State::Incompatible
            } else if !missing.is_empty() {
                State::Incomplete
            } else {
                State::Compatible
            };
            Cell {
                group: group.clone(),
                state,
                required: members.len(),
                observed: observed.into_iter().cloned().collect(),
                missing: missing.into_iter().cloned().collect(),
                below_threshold,
                below_member_af,
                minimum,
            }
        })
        .collect();
    let of = |wanted: &[State]| -> Vec<String> {
        cells
            .iter()
            .filter(|c| wanted.contains(&c.state))
            .map(|c| c.group.clone())
            .collect()
    };
    let compatible = of(&[State::Compatible]);
    let incomplete = of(&[State::Incomplete, State::Unassessable]);

    let mut reasons = Vec::new();
    let parent = &context.parent;
    if parent.status != "assigned"
        || parent.species_id.as_deref() != Some(PARENT_SPECIES_ID)
        || parent.subspecies_id.as_deref() != Some(PARENT_SUBSPECIES_ID)
        || !parent.verified
        || !ACCEPTED_PARENT_POLICIES.contains(&parent.policy_id.as_str())
    {
        reasons.push("absent_eligible_parent");
    }
    if context.input_kind != "assembly"
        || context.query_scope != QUERY_SCOPE
        || context.reference_scope != REFERENCE_SCOPE
    {
        reasons.push("unsupported_scope");
    }
    if context.engine != ENGINE
        || context.engine_version != ENGINE_VERSION
        || context.parameters != SEARCH_PARAMETERS
        || definition.version != DEFINITION_VERSION
    {
        reasons.push("unsupported_method_or_definition");
    }
    if context.search_status != "completed"
        || !context.adapter_verified
        || !crate::reference::is_sha256(&context.query_sha256)
        || context.query_sha256 != parent.query_sha256
        || !crate::reference::is_sha256(&context.package_sha256)
        || context.package_sha256 != parent.package_sha256
    {
        reasons.push("incomplete_or_unbound_execution");
    }
    if !["not_assessed", "explained_after_checks"].contains(&parent.scope_outcome.as_str()) {
        reasons.push("unresolved_scope");
    }
    if !parent.blocking_reasons.is_empty() {
        reasons.push("upstream_veto");
    }
    if context.diagnostic_exclusion {
        reasons.push("diagnostic_only_no_deployment_call");
    }
    if compatible.len() > 1 {
        reasons.push("overlapping_definitions");
    }
    if !incomplete.is_empty() {
        reasons.push("incomplete_alternative_or_member_evidence");
    }
    if compatible.is_empty() && incomplete.is_empty() {
        reasons.push("no_compatible_definition");
    }
    let selected = match compatible.as_slice() {
        [only] => cells.iter().find(|c| &c.group == only),
        _ => None,
    };
    if selected.is_some_and(|c| !c.below_member_af.is_empty()) {
        reasons.push("insufficient_member_coverage");
    }
    let mut warnings = vec![
        "Operational match to a frozen definition, not a population boundary or purity estimate.",
    ];
    if let Some(cell) = selected {
        if cell.required == 1 {
            warnings
                .push("Singleton definition: no independently demonstrated population boundary.");
        }
        if cell.minimum.is_some_and(|m| m.0 == ANI_MINIMUM) {
            warnings.push(
                "At the reported inclusive cutoff; unrounded ANI and estimator uncertainty unknown.",
            );
        }
    }
    let assigned = selected
        .filter(|_| reasons.is_empty())
        .map(|c| c.group.clone());
    Ok(Assessment {
        version: definition.version.clone(),
        membership_sha256: definition.membership_sha256(),
        member_count: definition.member_count(),
        assigned,
        compatible,
        incomplete,
        reasons,
        cells,
        warnings,
    })
}

/// v1, and the v2 successor when its call is the unchanged v1 skani call (see [`Parent`]).
pub const ACCEPTED_PARENT_POLICIES: [&str; 2] =
    [crate::decision::V1_POLICY_ID, crate::rescue::V2_POLICY_ID];
