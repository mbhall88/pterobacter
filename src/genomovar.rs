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
use crate::reference::{Rank, ReferenceComponent, VerifiedPackage};

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

/// The frozen definition's size and its [`Definition::membership_sha256`], computed from the
/// frozen membership table. The package must carry exactly this definition.
pub const FROZEN_GROUP_COUNT: usize = 25;
pub const FROZEN_MEMBER_COUNT: usize = 173;
pub const FROZEN_MEMBERSHIP_SHA256: &str =
    "1a2202dde4e0afcf78a89c1e6c084563169bd5cdea0f102da8113b1a87ebf0ab";
/// The [`member_source_sha256`] of the frozen historical chromosome sources.
pub const FROZEN_MEMBER_SOURCE_SHA256: &str =
    "757cfd93459593edb03a16fd7a150ce55c552ac8fe9e83c959bc897d4461b11d";

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

    /// The frozen definition, exactly as the authenticated package carries it: every genomovar
    /// concept at version [`DEFINITION_VERSION`] under `ECTX-SS007`, whose members are exactly
    /// the references of the `genomovar_chromosome` component, with the pinned group count,
    /// member count, membership digest and member source digest. Any other definition is
    /// refused, not adapted.
    pub fn from_package(package: &VerifiedPackage) -> Result<Self, GenomovarError> {
        let refuse = |why: String| {
            reject(format!(
                "the package does not carry the frozen genomovar definition {DEFINITION_VERSION}: {why}"
            ))
        };
        match package.concept(PARENT_SUBSPECIES_ID) {
            Some(c) if c.parent_id.as_deref() == Some(PARENT_SPECIES_ID) => {}
            _ => {
                return refuse(format!(
                    "{PARENT_SUBSPECIES_ID} is not a subspecies of {PARENT_SPECIES_ID}"
                ))
            }
        }
        let mut groups = BTreeMap::new();
        for concept in package
            .concepts()
            .iter()
            .filter(|c| c.rank == Rank::Genomovar)
        {
            let definition = match &concept.definition {
                Some(d)
                    if d.version == DEFINITION_VERSION
                        && d.sequence_scope == REFERENCE_SCOPE
                        && concept.parent_id.as_deref() == Some(PARENT_SUBSPECIES_ID) =>
                {
                    d
                }
                _ => return refuse(format!("{} is another definition", concept.id)),
            };
            groups.insert(concept.id.clone(), definition.member_ids.clone());
        }
        let component = match package.component(COMPONENT_ID) {
            Some(c)
                if c.engine == ENGINE
                    && c.engine_version == ENGINE_VERSION
                    && c.sequence_scope == REFERENCE_SCOPE =>
            {
                c
            }
            _ => return refuse(format!("no pinned {COMPONENT_ID} component")),
        };
        let definition = Definition::new(DEFINITION_VERSION, groups).or_else(|e| refuse(e.0))?;
        let mut members: Vec<&String> = definition.groups.values().flatten().collect();
        members.sort();
        let references: Vec<&String> = component
            .references
            .iter()
            .map(|r| &r.reference_id)
            .collect();
        if members != references {
            return refuse(format!(
                "the {COMPONENT_ID} references are not the definition members"
            ));
        }
        let sources = member_source_sha256(component);
        if sources != FROZEN_MEMBER_SOURCE_SHA256 {
            return refuse(format!("member source SHA-256 {sources}"));
        }
        if definition.groups.len() != FROZEN_GROUP_COUNT
            || definition.member_count() != FROZEN_MEMBER_COUNT
            || definition.membership_sha256() != FROZEN_MEMBERSHIP_SHA256
        {
            return refuse(format!(
                "{} groups of {} members with membership SHA-256 {}",
                definition.groups.len(),
                definition.member_count(),
                definition.membership_sha256()
            ));
        }
        Ok(definition)
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

/// SHA-256 of `reference_id<TAB>source_sha256<LF>` lines over a component's references, in ID
/// order: the identity of the sequences the definition was clustered on.
pub fn member_source_sha256(component: &ReferenceComponent) -> String {
    let mut lines: Vec<_> = component
        .references
        .iter()
        .map(|r| format!("{}\t{}\n", r.reference_id, r.source.sha256))
        .collect();
    lines.sort();
    format!("{:x}", Sha256::digest(lines.concat()))
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
pub struct GenomovarParent {
    pub policy_id: String,
    pub policy_sha256: String,
    pub result_sha256: String,
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

impl GenomovarParent {
    /// Read the parent from a final v1 or v2 decision, or a report built on one. Anything else
    /// gives an empty, unverified parent.
    ///
    /// Either call is verified only when the decision was bound to the authenticated package
    /// its evidence names (`reference_package.status: verified`). A v2 call must also cross the
    /// bridge to v1: default thresholds, a complete evaluation, a skani call basis, and the same
    /// species and subspecies as the v1 result on the same evidence. An ANIm rescue never
    /// qualifies, and its subspecies is null.
    pub fn from_report(report: &Value) -> Self {
        let text = |v: &Value| v.as_str().unwrap_or_default().to_owned();
        let id = |v: &Value| v.as_str().filter(|s| !s.is_empty()).map(str::to_owned);
        let reasons = |v: &Value| -> Vec<String> {
            v.as_array()
                .map(|a| a.iter().map(text).collect())
                .unwrap_or_else(|| vec!["missing_veto_record".into()])
        };
        let input = &report["input"];
        // Decided against the authenticated package its evidence names.
        let package = &report["reference_package"];
        let bound = package["status"] == "verified"
            && package["package"]["manifest_sha256"] == input["reference_package_sha256"];
        let mut parent = GenomovarParent {
            query_sha256: text(&input["query_sha256"]),
            package_sha256: text(&input["reference_package_sha256"]),
            scope_outcome: text(&report["scope_assessment"]["outcome"]),
            ..GenomovarParent::default()
        };
        if report["policy"]["id"] == crate::decision::V1_POLICY_ID {
            let a = &report["assignment"];
            parent.policy_id = crate::decision::V1_POLICY_ID.into();
            parent.status = text(&a["status"]);
            parent.species_id = id(&a["species_id"]);
            parent.subspecies_id = id(&a["subspecies_id"]);
            parent.blocking_reasons = reasons(&report["blocking_reasons"]);
            parent.verified = bound && report["evaluation_status"] == "completed";
        } else if report["policy_id"] == crate::rescue::V2_POLICY_ID {
            let legacy = &report["legacy_v1_result"];
            parent.policy_id = crate::rescue::V2_POLICY_ID.into();
            parent.status = text(&report["status"]);
            parent.species_id = id(&report["species_id"]);
            parent.subspecies_id = id(&report["subspecies_id"]);
            parent.blocking_reasons = reasons(&report["reasons"]);
            parent.verified = bound
                && report["policy_mode"] == "default_candidate"
                && report["evaluation_status"] == "complete"
                && report["call_basis"] == "skani"
                && legacy["policy_id"] == crate::decision::V1_POLICY_ID
                && legacy["status"] == "assigned"
                && id(&legacy["species_id"]) == parent.species_id
                && id(&legacy["subspecies_id"]) == parent.subspecies_id;
        }
        parent.policy_sha256 = parent_policy_sha256(&parent.policy_id);
        parent.result_sha256 = parent_result_sha256(report);
        parent
    }

    pub fn to_value(&self) -> Value {
        json!({"policy_id": self.policy_id, "policy_sha256": self.policy_sha256,
            "result_sha256": self.result_sha256, "status": self.status,
            "species_id": self.species_id, "subspecies_id": self.subspecies_id,
            "verified": self.verified, "query_sha256": self.query_sha256,
            "reference_package_sha256": self.package_sha256,
            "blocking_reasons": self.blocking_reasons, "scope_outcome": self.scope_outcome})
    }
}

/// The gates the parent alone decides. Any of them makes a genomovar search pointless.
fn parent_reasons(parent: &GenomovarParent) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    if parent.status != "assigned"
        || parent.species_id.as_deref() != Some(PARENT_SPECIES_ID)
        || parent.subspecies_id.as_deref() != Some(PARENT_SUBSPECIES_ID)
        || !parent.verified
        || !ACCEPTED_PARENT_POLICIES.contains(&parent.policy_id.as_str())
    {
        reasons.push("absent_eligible_parent");
    }
    if !["not_assessed", "explained_after_checks"].contains(&parent.scope_outcome.as_str()) {
        reasons.push("unresolved_scope");
    }
    if !parent.blocking_reasons.is_empty() {
        reasons.push("upstream_veto");
    }
    reasons
}

/// The withheld section for a query whose genomovar search is not run: the parent is not
/// eligible, or the package cannot support a call. `None` means the search should run.
pub fn before_search(
    parent: &GenomovarParent,
    definition: Result<&Definition, &str>,
) -> Option<Value> {
    let mut reasons = parent_reasons(parent);
    let detail = definition.err();
    if detail.is_some() {
        reasons.push("unsupported_method_or_definition");
    }
    (!reasons.is_empty()).then(|| {
        json!({"policy_id": POLICY_ID, "policy_sha256": reporting_policy_sha256(),
            "approval": approval(), "status": "withheld", "genomovar_id": null,
            "reasons": reasons, "detail": detail, "compatible_groups": [], "incomplete_groups": [],
            "groups": [], "warnings": [], "parent": parent.to_value(),
            "search": {"status": "not_run"}})
    })
}

/// The section for a genomovar search that failed: an execution failure, not a negative result.
pub fn failed(parent: &GenomovarParent, error: &str) -> Value {
    json!({"policy_id": POLICY_ID, "policy_sha256": reporting_policy_sha256(),
            "approval": approval(), "status": "failed", "genomovar_id": null,
        "reasons": ["incomplete_or_unbound_execution"], "detail": error,
        "compatible_groups": [], "incomplete_groups": [], "groups": [], "warnings": [],
        "parent": parent.to_value(), "search": {"status": "failed", "error": error}})
}

/// Recheck a report's genomovar immediately before publication, and only then set the report's
/// own `genomovar_id`. The section's parent must still be the report's final, complete and
/// eligible parent on the same query and package. Otherwise the genomovar is cleared and the
/// compatible evidence is kept.
pub fn recheck(report: &mut Value) {
    let section = &report["genomovar"];
    let mut assigned = section["genomovar_id"].as_str().map(str::to_owned);
    if assigned.is_some() {
        let parent = GenomovarParent::from_report(report);
        let complete = report
            .get("execution")
            .is_none_or(|e| e["status"] == "complete");
        let standing = complete
            && section["status"] == "assigned"
            && parent_reasons(&parent).is_empty()
            && parent.to_value() == section["parent"];
        if !standing {
            assigned = None;
            let section = &mut report["genomovar"];
            section["status"] = "withheld".into();
            section["genomovar_id"] = Value::Null;
            section["reasons"]
                .as_array_mut()
                .expect("assessment reasons")
                .push("parent_withdrawn_before_publication".into());
        }
    }
    let target = if report["assignment"].is_object() {
        &mut report["assignment"]
    } else {
        report
    };
    if target.is_object() {
        target["genomovar_id"] = assigned.into();
    }
}

/// What the front end's adapter verified about the parent and the genomovar search.
/// The default context is empty and fails closed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Context {
    pub parent: GenomovarParent,
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
    context: Context,
}

impl Assessment {
    pub fn assigned_genomovar_id(&self) -> Option<&str> {
        self.assigned.as_deref()
    }

    pub fn to_value(&self) -> Value {
        let pct = |p: Percent| serde_json::to_value(p).expect("percent");
        let c = &self.context;
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
        json!({"policy_id": POLICY_ID, "policy_sha256": reporting_policy_sha256(),
            "approval": approval(),
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
                "rival_exclusion": "measured_below_threshold_member"},
            "parent": c.parent.to_value(),
            "search": {"status": c.search_status, "input_kind": c.input_kind,
                "query_scope": c.query_scope, "reference_scope": c.reference_scope,
                "engine": c.engine, "engine_version": c.engine_version, "parameters": c.parameters,
                "adapter_verified": c.adapter_verified, "query_sha256": c.query_sha256,
                "reference_package_sha256": c.package_sha256,
                "diagnostic_exclusion": c.diagnostic_exclusion}})
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

    let parent = &context.parent;
    let mut reasons = parent_reasons(parent);
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
        context: context.clone(),
    })
}

/// v1, and the v2 successor when its call is the unchanged v1 skani call (see [`GenomovarParent`]).
pub const ACCEPTED_PARENT_POLICIES: [&str; 2] =
    [crate::decision::V1_POLICY_ID, crate::rescue::V2_POLICY_ID];

/// Reporting policy digest: exact UTF-8 base and accepted-addendum SHA-256 identities,
/// separated by a newline. These immutable contract documents define the policy.
pub fn reporting_policy_sha256() -> String {
    crate::evidence::sha256_hex(b"ae3eacf1a708a4c0fac18d81dc46df73c183d84accad4dd52e1c4a43de7e1b03\n1616c88a4be5193b8e54b6ce3ec27fcee19d731dd375c2e7ad4c989db429df4f\n")
}
fn approval() -> Value {
    json!({"status": "approved", "by": "Michael", "date": "2026-10-08",
        "decision": "Q-GV-AF-01 option A",
        "source": "https://github.com/mbhall88/Enterobacter/issues/3#issuecomment-6056920803",
        "base_contract_sha256": "ae3eacf1a708a4c0fac18d81dc46df73c183d84accad4dd52e1c4a43de7e1b03",
        "accepted_addendum_sha256": "1616c88a4be5193b8e54b6ce3ec27fcee19d731dd375c2e7ad4c989db429df4f"})
}
/// Parent implementation identity: exact policy source, with v2 also covering thresholds
/// and native-pair binding. Build paths and timestamps are excluded.
pub fn parent_policy_sha256(id: &str) -> String {
    let source = match id {
        crate::decision::V1_POLICY_ID => include_str!("decision.rs").to_owned(),
        crate::rescue::V2_POLICY_ID => [
            include_str!("decision.rs"),
            include_str!("rescue/mod.rs"),
            include_str!("rescue/thresholds.rs"),
            include_str!("rescue/records.rs"),
        ]
        .join("\n"),
        _ => return String::new(),
    };
    crate::evidence::sha256_hex(source.as_bytes())
}
/// Digest the parent decision before attachment. Runtime receipts, supplementary fields
/// and presentation-only report blocks (`versions`, and v1 `thresholds`) are excluded; decision
/// evidence identities, assignments, reasons and input binding remain.
pub fn parent_result_sha256(report: &Value) -> String {
    let mut decision = report.clone();
    if let Some(object) = decision.as_object_mut() {
        for key in [
            "genomovar",
            "mlst",
            "genomovar_id",
            "execution",
            "measurement",
            "evidence",
            "anim_execution",
            "anim_plan",
            "implementation",
        ] {
            object.remove(key);
        }
    }
    // Presentation added after publication checks: the version block always, and the v1 minima
    // (constants of the digested v1 policy source). v2's `thresholds` are decision evidence and stay.
    if let Some(object) = decision.as_object_mut() {
        object.remove("versions");
        if object.get("policy_id") != Some(&json!(crate::rescue::V2_POLICY_ID)) {
            object.remove("thresholds");
        }
    }
    if let Some(assignment) = decision
        .get_mut("assignment")
        .and_then(Value::as_object_mut)
    {
        assignment.remove("genomovar_id");
    }
    crate::evidence::sha256_hex(&serde_json::to_vec(&decision).expect("parent decision JSON"))
}
