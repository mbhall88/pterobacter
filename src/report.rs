//! Report formats. The JSON report is authoritative; the TSV and Markdown renderings here are
//! derived from it and never from the historical scaffold an earlier stage left behind. Pure:
//! reports go in as values and text comes out, so another front end can reuse it.
//!
//! A final call is read from exactly one place per policy: the `assignment` object under v1 and
//! the top-level call fields under v2. Labels appear as a call only when that place says
//! `assigned`. The nearest parent, the original v1 outcome and a witness are shown, but always
//! under a name that says they are not the call.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde_json::{json, Value};

use crate::decision::{Policy, V1_POLICY_ID};
use crate::rescue::REPORT_SCHEMA;

/// The three text renderings of one run's JSON reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// One row per assembly: final calls, reasons, policy, minima, method, counts, versions.
    pub calls_tsv: String,
    /// Long format: strongest matches, whole-panel parent summary and challenges per assembly.
    pub evidence_tsv: String,
    /// A readable review summary of every assembly.
    pub markdown: String,
}

const CALL_COLUMNS: &[&str] = &[
    "assembly",
    "input_file",
    "input_kind",
    "query_sha256",
    "total_bases",
    "record_count",
    "policy_id",
    "policy_mode",
    "validation_status",
    "execution_status",
    "execution_stage",
    "execution_error",
    "call_status",
    "species_id",
    "species_name",
    "subspecies_id",
    "subspecies_name",
    "genomovar_id",
    "reasons",
    "call_basis",
    "subspecies_reason",
    "original_v1_status",
    "original_v1_species_id",
    "original_v1_reasons",
    "trigger",
    "trigger_state",
    "rescue_status",
    "competitor_evaluation",
    "challenge_assessment",
    "anim_required",
    "anim_attempted",
    "anim_completed",
    "anim_no_alignment",
    "anim_failed",
    "anim_not_run",
    "skani_references",
    "skani_returned",
    "skani_not_returned",
    "min_species_ani",
    "min_subspecies_ani",
    "min_af",
    "subspecies_margin",
    "conflict_ani",
    "trigger_lower_ani",
    "skani_method",
    "anim_method",
    "implementation_version",
    "report_schema",
    "contract_revision",
    "package_version",
    "package_manifest_sha256",
    "evidence_sha256",
    "mlst_status",
    "mlst_state",
    "mlst_sequence_type",
];

const EVIDENCE_COLUMNS: &[&str] = &[
    "assembly",
    "section",
    "rank",
    "tie_group",
    "reference_id",
    "parent_kind",
    "parent_id",
    "reference_species_id",
    "reference_subspecies_id",
    "lineage_id",
    "reference_role",
    "skani_ani",
    "skani_af_query",
    "skani_af_reference",
    "anim_finished",
    "anim_identity_qr",
    "anim_identity_rq",
    "anim_orientations",
    "panel_references",
    "returned_references",
    "best_skani_reference_ids",
    "in_top",
    "challenging_reference_ids",
    "anim_unmeasured",
];

fn is_v2(report: &Value) -> bool {
    report["schema"] == REPORT_SCHEMA || report["policy_id"].is_string()
}

/// How far the foreign-parent challenge check was carried out. Absent measurements are not a
/// negative result, so only a finished run with a fully measured competitor panel is `completed`.
fn challenge_assessment(report: &Value) -> &'static str {
    if !is_v2(report) {
        return "not_applicable";
    }
    if report["execution"]["status"]
        .as_str()
        .is_some_and(|s| s != "complete")
    {
        return "not_assessed";
    }
    match report["competitor_evaluation"].as_str() {
        Some("complete") => "completed",
        Some("incomplete") => "partial",
        Some("not_required") => "not_applicable",
        _ => "not_assessed",
    }
}

/// A scalar as text. Numbers keep their emitted digits; lists of scalars join with `;`.
fn text(value: &Value) -> String {
    match value {
        Value::Null | Value::Object(_) => String::new(),
        Value::String(s) => s.clone(),
        Value::Array(items) => items.iter().map(text).collect::<Vec<_>>().join(";"),
        other => other.to_string(),
    }
}

/// One line of plain text: tabs and line breaks become spaces so a cell stays in its row.
fn cleaned(value: &str) -> String {
    value.replace(['\t', '\n', '\r'], " ")
}

/// The release identities a report carries, kept distinct: implementation, report schema,
/// policy and its contract, validation, measurement methods and reference data.
pub fn versions(report: &Value) -> Value {
    let v2 = is_v2(report);
    let package = &report["reference_package"]["package"];
    let policy = if v2 {
        text(&report["policy_id"])
    } else {
        text(&report["policy"]["id"])
    };
    let anim = [
        &report["anim_execution"]["method_profile"],
        &report["input"]["anim"]["method_profile"],
    ]
    .into_iter()
    .find(|v| v.is_string())
    .cloned()
    .unwrap_or(Value::Null);
    json!({
        "implementation": report["implementation"]["version"],
        "report_schema": report["schema"],
        "policy": policy,
        "policy_contract": if v2 { report["contract_revision"].clone() }
            else if policy == V1_POLICY_ID { json!("frozen-0.2.0") } else { Value::Null },
        "validation": if v2 { report["validation_status"].clone() } else { Value::Null },
        "skani_method": report["measurement"]["engine_profile"],
        "skani_engine_version": report["measurement"]["engine_version"],
        "anim_method": anim,
        "evidence_schema": report["input"]["evidence_schema"],
        "reference_data": {"package_id": package["id"], "package_version": package["version"],
            "schema_version": package["schema_version"], "manifest_sha256": package["manifest_sha256"]},
    })
}

/// Add the distinct version block, and the v1 minima that v2 already reports, to a finished
/// report. Nothing the decision produced is changed. Call it after the final publication
/// checks, because those compare against the report as decided.
pub fn annotate(report: &mut Value) {
    if !report.is_object() {
        return;
    }
    if !is_v2(report) && report["policy"]["id"] == V1_POLICY_ID && report["thresholds"].is_null() {
        report["thresholds"] = Policy::v1().thresholds_json();
    }
    report["versions"] = versions(report);
}

struct Call {
    status: String,
    species_id: String,
    species_name: String,
    subspecies_id: String,
    subspecies_name: String,
    genomovar_id: String,
    reasons: String,
    basis: String,
    subspecies_reason: String,
}

/// The final call, from its one authoritative place. A label survives only under `assigned`.
fn call(report: &Value) -> Call {
    let v2 = is_v2(report);
    let source = if v2 { report } else { &report["assignment"] };
    let status = match source["status"].as_str() {
        Some(s @ ("assigned" | "unresolved")) => s.to_owned(),
        _ => "not_evaluated".to_owned(),
    };
    let label = |key: &str| {
        if status == "assigned" {
            text(&source[key])
        } else {
            String::new()
        }
    };
    Call {
        species_id: label("species_id"),
        species_name: label("species_name"),
        subspecies_id: label("subspecies_id"),
        subspecies_name: label("subspecies_name"),
        genomovar_id: label("genomovar_id"),
        reasons: text(if v2 {
            &report["reasons"]
        } else {
            &report["blocking_reasons"]
        }),
        basis: if v2 {
            text(&report["call_basis"])
        } else {
            String::new()
        },
        subspecies_reason: text(&source["subspecies_reason"]),
        status,
    }
}

/// The original v1 outcome. Under v1 it is the call itself; under v2 it is `legacy_v1_result`.
fn original_v1(report: &Value, call: &Call) -> (String, String, String) {
    if is_v2(report) {
        let legacy = &report["legacy_v1_result"];
        if legacy.is_object() {
            let assigned = legacy["status"] == "assigned";
            return (
                text(&legacy["status"]),
                if assigned {
                    text(&legacy["species_id"])
                } else {
                    String::new()
                },
                text(&legacy["reasons"]),
            );
        }
        return Default::default();
    }
    (
        call.status.clone(),
        call.species_id.clone(),
        call.reasons.clone(),
    )
}

fn summary(index: usize, report: &Value) -> BTreeMap<&'static str, String> {
    let call = call(report);
    let (v1_status, v1_species, v1_reasons) = original_v1(report, &call);
    let v = versions(report);
    let thresholds = &report["thresholds"];
    let counts = &report["counts"];
    let evidence = &report["evidence"];
    let mlst = &report["mlst"];
    let mut row: BTreeMap<&'static str, String> = BTreeMap::new();
    let mut put = |key: &'static str, value: String| {
        row.insert(key, cleaned(&value));
    };
    put("assembly", (index + 1).to_string());
    put("input_file", text(&report["input"]["file"]));
    put("input_kind", text(&report["input"]["kind"]));
    put("query_sha256", text(&report["input"]["query_sha256"]));
    put("total_bases", text(&report["input"]["total_bases"]));
    put(
        "record_count",
        report["input"]["records"]
            .as_array()
            .map_or_else(String::new, |r| r.len().to_string()),
    );
    put("policy_id", text(&v["policy"]));
    put("policy_mode", text(&report["policy_mode"]));
    put("validation_status", text(&v["validation"]));
    put("execution_status", text(&report["execution"]["status"]));
    put("execution_stage", text(&report["execution"]["stage"]));
    put("execution_error", text(&report["execution"]["error"]));
    put("call_status", call.status.clone());
    put("species_id", call.species_id.clone());
    put("species_name", call.species_name.clone());
    put("subspecies_id", call.subspecies_id.clone());
    put("subspecies_name", call.subspecies_name.clone());
    put("genomovar_id", call.genomovar_id.clone());
    put("reasons", call.reasons.clone());
    put("call_basis", call.basis.clone());
    put("subspecies_reason", call.subspecies_reason.clone());
    put("original_v1_status", v1_status);
    put("original_v1_species_id", v1_species);
    put("original_v1_reasons", v1_reasons);
    put("trigger", text(&report["trigger"]));
    put("trigger_state", text(&report["trigger_state"]));
    put("rescue_status", text(&report["rescue_status"]));
    put(
        "competitor_evaluation",
        text(&report["competitor_evaluation"]),
    );
    put(
        "challenge_assessment",
        challenge_assessment(report).to_owned(),
    );
    for (column, key) in [
        ("anim_required", "required"),
        ("anim_attempted", "attempted"),
        ("anim_completed", "completed"),
        ("anim_no_alignment", "no_alignment"),
        ("anim_failed", "failed"),
        ("anim_not_run", "not_run"),
    ] {
        put(column, text(&counts[key]));
    }
    put("skani_references", text(&evidence["reference_count"]));
    put("skani_returned", text(&evidence["raw_hit_count"]));
    put(
        "skani_not_returned",
        text(&evidence["not_returned_reference_count"]),
    );
    put("min_species_ani", text(&thresholds["species_ani"]));
    put("min_subspecies_ani", text(&thresholds["subspecies_ani"]));
    put("min_af", text(&thresholds["both_af_minimum"]));
    put(
        "subspecies_margin",
        text(&thresholds["subspecies_margin_percentage_points"]),
    );
    put("conflict_ani", text(&thresholds["conflict_ani"]));
    put("trigger_lower_ani", text(&thresholds["trigger_lower_ani"]));
    put("skani_method", text(&v["skani_method"]));
    put("anim_method", text(&v["anim_method"]));
    put("implementation_version", text(&v["implementation"]));
    put("report_schema", text(&v["report_schema"]));
    put("contract_revision", text(&v["policy_contract"]));
    put(
        "package_version",
        text(&v["reference_data"]["package_version"]),
    );
    put(
        "package_manifest_sha256",
        text(&v["reference_data"]["manifest_sha256"]),
    );
    put("evidence_sha256", text(&report["input"]["evidence_sha256"]));
    put("mlst_status", text(&mlst["status"]));
    put("mlst_state", text(&mlst["state"]));
    put("mlst_sequence_type", text(&mlst["sequence_type"]));
    row
}

fn tsv_row(columns: &[&str], row: &BTreeMap<&str, String>) -> String {
    let cells: Vec<&str> = columns
        .iter()
        .map(|c| row.get(c).map_or("", String::as_str))
        .collect();
    cells.join("\t")
}

fn parent_of(entry: &Value) -> (String, String) {
    let parent = &entry["parent"];
    let id = if parent["concept_id"].is_string() {
        text(&parent["concept_id"])
    } else {
        text(&parent["reference_id"])
    };
    (text(&parent["kind"]), id)
}

fn evidence_rows(index: usize, report: &Value) -> Vec<BTreeMap<&'static str, String>> {
    let mut rows = Vec::new();
    let base = |section: &str| {
        let mut row: BTreeMap<&'static str, String> = BTreeMap::new();
        row.insert("assembly", (index + 1).to_string());
        row.insert("section", section.to_owned());
        row
    };
    for (rank, entry) in report["top_references"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let mut row = base("top_reference");
        let (kind, id) = parent_of(entry);
        let anim = &entry["anim"];
        row.insert("rank", (rank + 1).to_string());
        row.insert("tie_group", text(&entry["tie_group"]));
        row.insert("reference_id", text(&entry["reference_id"]));
        row.insert("parent_kind", kind);
        row.insert("parent_id", id);
        row.insert("reference_species_id", text(&entry["species_id"]));
        row.insert("reference_subspecies_id", text(&entry["subspecies_id"]));
        row.insert("lineage_id", text(&entry["lineage_id"]));
        row.insert("reference_role", text(&entry["reference_role"]));
        row.insert("skani_ani", text(&entry["skani"]["ani_percent"]));
        row.insert("skani_af_query", text(&entry["skani"]["af_query_percent"]));
        row.insert(
            "skani_af_reference",
            text(&entry["skani"]["af_reference_percent"]),
        );
        row.insert("anim_finished", text(&anim["finished"]));
        row.insert("anim_identity_qr", text(&anim["QR"]["identity_percent"]));
        row.insert("anim_identity_rq", text(&anim["RQ"]["identity_percent"]));
        rows.push(row);
    }
    for entry in report["panel_summary"].as_array().into_iter().flatten() {
        let mut row = base("panel_parent");
        let (kind, id) = parent_of(entry);
        row.insert("parent_kind", kind);
        row.insert("parent_id", id);
        row.insert("panel_references", text(&entry["panel_reference_count"]));
        row.insert(
            "returned_references",
            text(&entry["returned_reference_count"]),
        );
        row.insert("skani_ani", text(&entry["best_skani_ani_percent"]));
        row.insert(
            "best_skani_reference_ids",
            text(&entry["best_skani_reference_ids"]),
        );
        row.insert("in_top", text(&entry["in_top_references"]));
        row.insert(
            "challenging_reference_ids",
            text(&entry["challenging_reference_ids"]),
        );
        row.insert(
            "anim_unmeasured",
            text(&entry["anim_unmeasured_reference_count"]),
        );
        rows.push(row);
    }
    for entry in report["challenge_evidence"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let mut row = base("challenge");
        let (kind, id) = parent_of(entry);
        row.insert("reference_id", text(&entry["reference_id"]));
        row.insert("parent_kind", kind);
        row.insert("parent_id", id);
        row.insert("skani_ani", text(&entry["skani"]["ani_percent"]));
        row.insert("skani_af_query", text(&entry["skani"]["af_query_percent"]));
        row.insert(
            "skani_af_reference",
            text(&entry["skani"]["af_reference_percent"]),
        );
        row.insert(
            "anim_orientations",
            text(&entry["anim_challenging_orientations"]),
        );
        rows.push(row);
    }
    rows
}

fn cell(value: &str) -> String {
    cleaned(value).replace('|', "\\|")
}

fn or_dash(value: &str) -> &str {
    if value.is_empty() {
        "—"
    } else {
        value
    }
}

fn markdown_assembly(index: usize, report: &Value, s: &BTreeMap<&'static str, String>) -> String {
    let g = |key: &str| s.get(key).map_or("", String::as_str);
    let mut md = String::new();
    let _ = writeln!(
        md,
        "## Assembly {}: {}\n",
        index + 1,
        or_dash(g("input_file"))
    );
    let rank_line = |label: &str, id: &str, name: &str, status: &str| match status {
        "assigned" => format!("**{label}:** assigned — {} ({id})", or_dash(name)),
        "unresolved" => format!("**{label}:** unresolved (no assignment)"),
        _ => format!("**{label}:** not evaluated"),
    };
    let _ = writeln!(md, "### Final call\n");
    let _ = writeln!(md, "**Call status:** {}\n", g("call_status"));
    let _ = writeln!(
        md,
        "- {}",
        rank_line(
            "Species",
            g("species_id"),
            g("species_name"),
            g("call_status")
        )
    );
    let sub_status = if g("subspecies_id").is_empty() && g("call_status") == "assigned" {
        "unresolved"
    } else {
        g("call_status")
    };
    let _ = writeln!(
        md,
        "- {}",
        rank_line(
            "Subspecies",
            g("subspecies_id"),
            g("subspecies_name"),
            sub_status
        )
    );
    let genomovar = if g("genomovar_id").is_empty() {
        "none reported".to_owned()
    } else {
        g("genomovar_id").to_owned()
    };
    let _ = writeln!(md, "- **Genomovar:** {genomovar}");
    let _ = writeln!(
        md,
        "- **Reasons:** {}",
        or_dash(&g("reasons").replace(';', ", "))
    );
    if !g("subspecies_reason").is_empty() {
        let _ = writeln!(md, "- **Subspecies reason:** {}", g("subspecies_reason"));
    }
    if !g("call_basis").is_empty() {
        let _ = writeln!(md, "- **Call basis:** {}", g("call_basis"));
    }
    let nearest = nearest_line(report);
    if !nearest.is_empty() {
        let _ = writeln!(md, "- **Nearest parent (not an assignment):** {nearest}");
    }
    let _ = writeln!(md, "\n### Policy and execution\n");
    let _ = writeln!(md, "**Policy:** {}\n", g("policy_id"));
    let _ = writeln!(md, "- Policy mode: {}", or_dash(g("policy_mode")));
    let _ = writeln!(
        md,
        "- Validation status: {}",
        or_dash(g("validation_status"))
    );
    let _ = writeln!(
        md,
        "- Minimum species ANI: {}",
        or_dash(g("min_species_ani"))
    );
    let _ = writeln!(
        md,
        "- Minimum subspecies ANI: {}",
        or_dash(g("min_subspecies_ani"))
    );
    let _ = writeln!(
        md,
        "- Minimum aligned fraction (both directions): {}",
        or_dash(g("min_af"))
    );
    let _ = writeln!(
        md,
        "- Subspecies margin (percentage points): {}",
        or_dash(g("subspecies_margin"))
    );
    if !g("conflict_ani").is_empty() {
        let _ = writeln!(md, "- Conflict ANI: {}", g("conflict_ani"));
        let _ = writeln!(
            md,
            "- Trigger lower ANI: {}",
            or_dash(g("trigger_lower_ani"))
        );
    }
    let _ = writeln!(
        md,
        "- Whole-assembly skani method: {}",
        or_dash(g("skani_method"))
    );
    if !g("anim_method").is_empty() {
        let _ = writeln!(md, "- ANIm method: {}", g("anim_method"));
    }
    let _ = writeln!(
        md,
        "- Execution: {} at stage {}",
        or_dash(g("execution_status")),
        or_dash(g("execution_stage"))
    );
    if !g("execution_error").is_empty() {
        let _ = writeln!(md, "- Execution error: {}", g("execution_error"));
    }
    if !is_v2(report) {
        let _ = writeln!(md, "\n### Original v1 outcome and rescue\n");
        let _ = writeln!(
            md,
            "- Original v1 outcome: this report is the v1 outcome; no rescue applies."
        );
        let _ = writeln!(md, "- ANIm: not applicable under this policy");
    }
    if is_v2(report) {
        let _ = writeln!(md, "\n### Original v1 outcome and rescue\n");
        let _ = writeln!(
            md,
            "- Original v1 outcome: {} (species {}; reasons {})",
            or_dash(g("original_v1_status")),
            or_dash(g("original_v1_species_id")),
            or_dash(&g("original_v1_reasons").replace(';', ", "))
        );
        let _ = writeln!(
            md,
            "- Trigger: {} ({})",
            or_dash(g("trigger")),
            or_dash(g("trigger_state"))
        );
        let _ = writeln!(md, "- Rescue state: {}", or_dash(g("rescue_status")));
        let _ = writeln!(
            md,
            "- Competitor panel: {}",
            or_dash(g("competitor_evaluation"))
        );
        let _ = writeln!(md, "- Challenge check: {}", g("challenge_assessment"));
        let _ = writeln!(
            md,
            "- ANIm pairs: {} required, {} attempted, {} completed, {} no alignment, {} failed, {} not run",
            or_dash(g("anim_required")),
            or_dash(g("anim_attempted")),
            or_dash(g("anim_completed")),
            or_dash(g("anim_no_alignment")),
            or_dash(g("anim_failed")),
            or_dash(g("anim_not_run"))
        );
    }
    let _ = writeln!(
        md,
        "- skani panel: {} references, {} returned, {} not returned",
        or_dash(g("skani_references")),
        or_dash(g("skani_returned")),
        or_dash(g("skani_not_returned"))
    );
    let _ = writeln!(md, "\n### Strongest matches\n");
    let _ = writeln!(
        md,
        "The five highest skani references are shown, plus every exact tie at fifth place. \
         Rows in one tie group are ordered by reference ID for display only; the order does not \
         choose a winner. Each reference is evidence, not a vote. ANI methods are not mixed in \
         one ranking: skani orders this table and any ANIm values are shown beside it.\n"
    );
    let top = report["top_references"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if top.is_empty() {
        let _ = writeln!(md, "No skani reference was returned.\n");
    } else {
        let _ = writeln!(
            md,
            "| Tie group | Reference | Parent | skani ANI % | AF query % | AF reference % | ANIm identity QR/RQ % |"
        );
        let _ = writeln!(md, "|---|---|---|---|---|---|---|");
        for entry in &top {
            let (kind, id) = parent_of(entry);
            let anim = &entry["anim"];
            let identity = if anim.is_object() {
                format!(
                    "{} / {}",
                    text(&anim["QR"]["identity_percent"]),
                    text(&anim["RQ"]["identity_percent"])
                )
            } else {
                "—".into()
            };
            let _ = writeln!(
                md,
                "| {} | {} | {} {} | {} | {} | {} | {} |",
                text(&entry["tie_group"]),
                cell(&text(&entry["reference_id"])),
                cell(&kind),
                cell(&id),
                text(&entry["skani"]["ani_percent"]),
                text(&entry["skani"]["af_query_percent"]),
                text(&entry["skani"]["af_reference_percent"]),
                identity
            );
        }
        let _ = writeln!(md);
    }
    let _ = writeln!(md, "### Whole-panel species and lineage summary\n");
    let panel = report["panel_summary"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if panel.is_empty() {
        let _ = writeln!(md, "No panel summary was produced for this report.\n");
    } else {
        let _ = writeln!(
            md,
            "| Parent | Panel references | Returned | Best skani ANI % | Among strongest matches | Challenging references |"
        );
        let _ = writeln!(md, "|---|---|---|---|---|---|");
        for entry in &panel {
            let (kind, id) = parent_of(entry);
            let _ = writeln!(
                md,
                "| {} {} | {} | {} | {} | {} | {} |",
                cell(&kind),
                cell(&id),
                text(&entry["panel_reference_count"]),
                text(&entry["returned_reference_count"]),
                or_dash(&text(&entry["best_skani_ani_percent"])),
                if entry["in_top_references"] == true {
                    "yes"
                } else {
                    "no"
                },
                or_dash(&cell(&text(&entry["challenging_reference_ids"])).replace(';', ", "))
            );
        }
        let _ = writeln!(md);
    }
    let _ = writeln!(md, "### Challenges\n");
    let shown: BTreeSet<String> = top.iter().map(|e| text(&e["reference_id"])).collect();
    let challenges = report["challenge_evidence"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if !is_v2(report) {
        let _ = writeln!(
            md,
            "The v1 policy has no foreign-parent challenge test, so none is listed. Every other \
             parent's best skani ANI is in the whole-panel summary above; compare it with the call.\n"
        );
    } else {
        let note = match challenge_assessment(report) {
            "completed" if challenges.is_empty() => {
                Some("No reference challenged the call. The competitor panel was fully measured.")
            }
            "completed" => None,
            "partial" => Some(
                "Partially assessed: some competitor measurements are missing, so a reference not \
                 listed here has not been shown to leave the call unchallenged.",
            ),
            "not_applicable" => Some(
                "Not applicable: no rescue was attempted, so no challenge test ran.",
            ),
            _ => Some(
                "Not assessed: the run stopped before the competitor panel was measured, or the \
                 policy did not need it. The absence of a listed challenge is not a negative result.",
            ),
        };
        if let Some(note) = note {
            let _ = writeln!(md, "{note}\n");
        }
        for entry in &challenges {
            let reference = text(&entry["reference_id"]);
            let (kind, id) = parent_of(entry);
            let place = if shown.contains(&reference) {
                "among the strongest matches"
            } else {
                "outside the strongest matches"
            };
            let orientations = text(&entry["anim_challenging_orientations"]).replace(';', ", ");
            let _ = writeln!(
                md,
                "- `{reference}` ({kind} {id}), {place}: skani ANI {} %, AF {} %; ANIm challenging orientations: {}",
                text(&entry["skani"]["ani_percent"]),
                text(&entry["skani"]["af_query_percent"]),
                or_dash(&orientations)
            );
        }
        if !challenges.is_empty() {
            let _ = writeln!(md);
        }
    }
    let _ = writeln!(md, "### Supplementary results\n");
    let _ = writeln!(md, "- Historical MLST: {}", or_dash(g("mlst_status")));
    if !g("mlst_sequence_type").is_empty() || !g("mlst_state").is_empty() {
        let _ = writeln!(
            md,
            "  - state {}, sequence type {} (supplementary; it never sets a taxon)",
            or_dash(g("mlst_state")),
            or_dash(g("mlst_sequence_type"))
        );
    }
    let genomovar_status = text(&report["genomovar"]["status"]);
    let _ = writeln!(md, "- Genomovar search: {}", or_dash(&genomovar_status));
    let _ = writeln!(md, "\n### Provenance\n");
    let _ = writeln!(md, "- Query SHA-256: `{}`", g("query_sha256"));
    let _ = writeln!(
        md,
        "- Bases: {}; records: {}",
        or_dash(g("total_bases")),
        or_dash(g("record_count"))
    );
    let _ = writeln!(md, "- Evidence SHA-256: {}", or_dash(g("evidence_sha256")));
    let _ = writeln!(
        md,
        "- Reference package {}, manifest SHA-256 {}",
        or_dash(g("package_version")),
        or_dash(g("package_manifest_sha256"))
    );
    let _ = writeln!(
        md,
        "- Versions: implementation {}; report schema {}; policy {} ({}); skani method {}; ANIm method {}",
        or_dash(g("implementation_version")),
        or_dash(g("report_schema")),
        or_dash(g("policy_id")),
        or_dash(g("contract_revision")),
        or_dash(g("skani_method")),
        or_dash(g("anim_method"))
    );
    let _ = writeln!(md, "\n### What to do next\n");
    let next = match g("call_status") {
        "assigned" => {
            "The assignment is conditional on the approved rules and the supplied sequence. It does not establish that the isolate is pure."
        }
        "unresolved" => {
            "No assignment was made. Read the reasons, compare the strongest matches and any challenges, then follow the manual-review steps in the usage guide (docs/usage.md): a tree and an ANI check against the named references. The software does not invent a label."
        }
        _ => {
            "The analysis did not complete, so no biological conclusion can be drawn from this report. Fix the execution error above and run again; retained evidence is in the JSON report."
        }
    };
    let _ = writeln!(md, "{next}\n");
    md
}

/// The nearest parent as evidence: the v2 field when set, else the references tied for the
/// highest skani ANI. Never the call.
fn nearest_line(report: &Value) -> String {
    let parent = &report["nearest_parent"];
    if parent.is_object() && !parent["concept_id"].is_null() {
        let name = text(&parent["name"]);
        let id = text(&parent["concept_id"]);
        let kind = text(&parent["kind"]);
        let max = text(&report["max_skani_ani_percent"]);
        let ani = if max.is_empty() {
            String::new()
        } else {
            format!(", skani ANI {max} %")
        };
        return if name.is_empty() {
            format!("{kind} {id}{ani}")
        } else {
            format!("{name} ({kind} {id}){ani}")
        };
    }
    let tied: Vec<&Value> = report["top_references"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| e["tie_group"] == 0)
        .collect();
    let Some(first) = tied.first() else {
        return String::new();
    };
    let parents: BTreeSet<String> = tied
        .iter()
        .map(|e| {
            let (kind, id) = parent_of(e);
            format!("{kind} {id}")
        })
        .collect();
    format!(
        "{}, skani ANI {} %",
        parents.into_iter().collect::<Vec<_>>().join(" / "),
        text(&first["skani"]["ani_percent"])
    )
}

/// Render the TSV and Markdown reports for the JSON reports of one run, in input order.
pub fn render(reports: &[Value]) -> Rendered {
    let mut calls = CALL_COLUMNS.join("\t");
    calls.push('\n');
    let mut evidence = EVIDENCE_COLUMNS.join("\t");
    evidence.push('\n');
    let mut markdown = String::from("# Terobacter report\n\n");
    let _ = writeln!(
        markdown,
        "This summary is derived from the JSON report, which is authoritative. Assemblies: {}.\n",
        reports.len()
    );
    for (index, report) in reports.iter().enumerate() {
        let row = summary(index, report);
        calls.push_str(&tsv_row(CALL_COLUMNS, &row));
        calls.push('\n');
        for entry in evidence_rows(index, report) {
            let one_line: BTreeMap<&str, String> =
                entry.into_iter().map(|(k, v)| (k, cleaned(&v))).collect();
            evidence.push_str(&tsv_row(EVIDENCE_COLUMNS, &one_line));
            evidence.push('\n');
        }
        markdown.push_str(&markdown_assembly(index, report, &row));
    }
    Rendered {
        calls_tsv: calls,
        evidence_tsv: evidence,
        markdown,
    }
}
