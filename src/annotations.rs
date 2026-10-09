//! Optional frozen v1 method and problem-warning envelopes. Contract validation is pure;
//! front ends verify the external artifact bytes before reporting success.
use crate::evidence::{is_sha256, EvidenceError, Percent, SavedEvidence};
use serde_json::{json, Value};
use std::collections::HashSet;

fn require(ok: bool, message: &str) -> Result<(), EvidenceError> {
    if ok {
        Ok(())
    } else {
        Err(EvidenceError::Invalid(message.to_owned()))
    }
}
fn text(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.trim().is_empty())
}
fn exact(v: &Value, keys: &[&str], label: &str) -> Result<(), EvidenceError> {
    require(
        v.as_object()
            .is_some_and(|m| m.len() == keys.len() && keys.iter().all(|k| m.contains_key(*k))),
        label,
    )
}
fn items<'a>(
    envelope: &'a Value,
    key: &str,
    schema: &str,
    evidence: &SavedEvidence,
) -> Result<&'a [Value], EvidenceError> {
    exact(
        envelope,
        &[
            "schema_version",
            "query_sha256",
            "evidence_sha256",
            "reference_package_sha256",
            key,
        ],
        "invalid annotation envelope fields",
    )?;
    require(
        envelope["schema_version"] == schema
            && envelope["query_sha256"] == evidence.provenance().query_sha256
            && envelope["reference_package_sha256"]
                == evidence.provenance().reference_package_sha256
            && envelope["evidence_sha256"] == evidence.source_sha256(),
        "annotation binding mismatch",
    )?;
    envelope[key]
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| EvidenceError::Invalid("annotation list required".into()))
}
fn artifacts(v: &Value) -> Result<(), EvidenceError> {
    let Some(pins) = v.as_array() else {
        return require(false, "source artifacts required");
    };
    require(!pins.is_empty(), "source artifacts required")?;
    for pin in pins {
        exact(pin, &["path", "sha256"], "invalid artifact fields")?;
        require(
            pin["path"]
                .as_str()
                .is_some_and(|s| s.starts_with('/') && s.len() > 1)
                && pin["sha256"].as_str().is_some_and(is_sha256),
            "invalid artifact pin",
        )?;
    }
    Ok(())
}

/// Compare a finite decimal percentage to a whole-percent gate without f64 rounding.
/// ANIm diagnostics need not use skani's two-decimal quantisation.
fn metric_at_least(v: &Value, gate: u32) -> Result<bool, EvidenceError> {
    let literal = match v {
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => {
            return Err(EvidenceError::Invalid(
                "explicit decimal method percentage required".into(),
            ))
        }
    };
    let (mantissa, exponent) = if let Some((m, e)) = literal.split_once(['e', 'E']) {
        (
            m,
            e.parse::<i64>()
                .map_err(|_| EvidenceError::Invalid("invalid method exponent".into()))?,
        )
    } else {
        (literal.as_str(), 0)
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    require(
        !whole.is_empty()
            && whole.bytes().all(|c| c.is_ascii_digit())
            && fraction.bytes().all(|c| c.is_ascii_digit()),
        "finite nonnegative method percentage required",
    )?;
    let digits = format!("{whole}{fraction}");
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Ok(gate == 0);
    }
    // Number of integral digits once exponent and discarded leading zeroes are accounted for.
    let width = i128::try_from(digits.len()).expect("string length fits i128")
        - i128::try_from(fraction.len()).expect("string length fits i128")
        + i128::from(exponent);
    require(width <= 3, "method percentage outside [0,100]")?;
    if width == 3 {
        let padded = format!("{digits:0<3}");
        require(
            padded.starts_with("100") && padded[3..].bytes().all(|c| c == b'0'),
            "method percentage outside [0,100]",
        )?;
        return Ok(true);
    }
    if width <= 0 {
        return Ok(gate == 0);
    }
    let width = usize::try_from(width).expect("positive width at most two");
    let integer: u32 = format!("{digits:0<width$}")[..width]
        .parse()
        .expect("decimal digits");
    Ok(integer >= gate)
}
fn gate(triple: &Value, direction: bool) -> Result<bool, EvidenceError> {
    let mut keys = vec!["ani_percent", "af_query_percent", "af_reference_percent"];
    if direction {
        keys.push("direction_id");
    }
    exact(triple, &keys, "invalid method triple fields")?;
    let ani = metric_at_least(&triple["ani_percent"], 96)?;
    let aq = metric_at_least(&triple["af_query_percent"], 80)?;
    let ar = metric_at_least(&triple["af_reference_percent"], 80)?;
    Ok(ani && aq && ar)
}
fn warnings(envelope: &Value, evidence: &SavedEvidence) -> Result<Vec<Value>, EvidenceError> {
    let mut ids = HashSet::new();
    items(
        envelope,
        "warnings",
        "1.0-shadow-problem-warnings",
        evidence,
    )?
    .iter()
    .map(|w| {
        exact(
            w,
            &[
                "warning_id",
                "reason",
                "source",
                "source_version",
                "artifacts",
            ],
            "invalid warning fields",
        )?;
        require(
            ["warning_id", "reason", "source", "source_version"]
                .iter()
                .all(|k| text(&w[k]))
                && ids.insert(w["warning_id"].as_str()),
            "invalid or duplicate warning metadata",
        )?;
        artifacts(&w["artifacts"])?;
        let mut out = w.clone();
        out["interpretation"] = "supplied problem warning; not measured detector success".into();
        out["blocks_shadow"] = true.into();
        Ok(out)
    })
    .collect()
}
fn diagnostics(
    envelope: &Value,
    evidence: &SavedEvidence,
    inventory: &Value,
) -> Result<Vec<Value>, EvidenceError> {
    exact(
        inventory,
        &[
            "schema_version",
            "package_sha256",
            "component_id",
            "references",
            "representation",
        ],
        "invalid inventory fields",
    )?;
    require(
        inventory["schema_version"] == "1.0-shadow-primary-inventory"
            && inventory["package_sha256"] == evidence.provenance().reference_package_sha256
            && inventory["component_id"] == "primary_whole",
        "invalid primary inventory binding",
    )?;
    let refs = inventory["references"]
        .as_array()
        .ok_or_else(|| EvidenceError::Invalid("inventory references required".into()))?;
    require(
        refs.len() == evidence.reference_count(),
        "inventory count mismatch",
    )?;
    let mut inventory_ids = HashSet::new();
    for r in refs {
        require(
            text(&r["reference_id"])
                && inventory_ids.insert(r["reference_id"].as_str())
                && r["source_sha256"].as_str().is_some_and(is_sha256),
            "invalid or duplicate inventory reference",
        )?;
    }
    for hit in evidence.references() {
        let r = refs
            .iter()
            .find(|r| r["reference_id"] == hit.id)
            .ok_or_else(|| EvidenceError::Invalid("reference outside primary inventory".into()))?;
        for (key, expected) in [
            (
                "species_id",
                hit.species.as_ref().map_or("", |s| s.id.as_str()),
            ),
            (
                "species_name",
                hit.species.as_ref().map_or("", |s| s.name.as_str()),
            ),
            (
                "subspecies_id",
                hit.subspecies.as_ref().map_or("", |s| s.id.as_str()),
            ),
            (
                "subspecies_name",
                hit.subspecies.as_ref().map_or("", |s| s.name.as_str()),
            ),
            ("lineage_id", hit.lineage_id.as_deref().unwrap_or("")),
            ("name_status", hit.name_status.as_str()),
            ("reference_role", hit.reference_role.as_str()),
        ] {
            require(r[key] == expected, "inventory concept/name/role mismatch")?;
        }
    }
    let mut ids = HashSet::new();
    items(envelope, "diagnostics", "1.0-shadow-diagnostics", evidence)?.iter().map(|d| {
        exact(d, &["diagnostic_id","method","method_version","reference_id","reference_source_sha256",
            "query_scope","reference_scope","skani_version","skani","directions","artifacts"], "invalid diagnostic fields")?;
        require(text(&d["diagnostic_id"]) && ids.insert(d["diagnostic_id"].as_str())
            && d["method"] == "reciprocal_ANIm" && text(&d["method_version"]), "duplicate or invalid diagnostic method")?;
        require(d["query_scope"] == "whole_assembly_as_supplied" && d["reference_scope"] == "whole_assembly_as_deposited"
            && d["skani_version"] == evidence.provenance().engine_version, "diagnostic scope/version mismatch")?;
        let hit = evidence.references().iter().find(|r| d["reference_id"] == r.id)
            .ok_or_else(|| EvidenceError::Invalid("diagnostic reference absent from returned skani evidence".into()))?;
        let reference = refs.iter().find(|r| r["reference_id"] == hit.id).expect("inventory validated");
        require(d["reference_source_sha256"] == reference["source_sha256"], "diagnostic reference hash mismatch")?;
        exact(&d["skani"], &["ani_percent","af_query_percent","af_reference_percent"], "invalid skani triple fields")?;
        for (key, expected) in [("ani_percent",hit.ani),("af_query_percent",hit.af_query),("af_reference_percent",hit.af_reference)] {
            let value: Percent = serde_json::from_value(d["skani"][key].clone())?;
            require(value == expected, "diagnostic skani triple mismatch")?;
        }
        let skani_pass = hit.ani >= Percent::from_whole(96) && hit.min_af() >= Percent::from_whole(80);
        let directions = d["directions"].as_array().ok_or_else(|| EvidenceError::Invalid("reciprocal directions required".into()))?;
        require(directions.len() == 2 && directions.iter().map(|x| x["direction_id"].as_str()).collect::<HashSet<_>>()
            == HashSet::from([Some("reference_to_query"),Some("query_to_reference")]), "two unique reciprocal directions required")?;
        let mut observed = Vec::new();
        let mut disagreement = false;
        for direction in directions {
            let passes = gate(direction, true)?;
            disagreement |= passes != skani_pass;
            let mut out = direction.clone(); out["meets_same_species_gate"] = passes.into(); observed.push(out);
        }
        artifacts(&d["artifacts"])?;
        Ok(json!({"diagnostic_id":d["diagnostic_id"],"reference_id":d["reference_id"],"method":d["method"],
            "method_version":d["method_version"],"skani":d["skani"],"skani_meets_same_species_gate":skani_pass,
            "directions":observed,"gate_disagreement":disagreement,
            "relevant_to_nearest_parent": evidence.references().first().is_some_and(|top| top.species.is_some() && hit.species.as_ref().map(|s|&s.id) == top.species.as_ref().map(|s|&s.id)),
            "blocks_shadow":disagreement,"artifacts":d["artifacts"],
            "interpretation":"method-dependent comparison; neither estimator is universal truth"}))
    }).collect()
}
impl SavedEvidence {
    /// Attach optional v1 envelopes bound to these exact saved bytes. Diagnostics additionally
    /// require primary reference metadata/source hashes from a checked package. This function
    /// validates contracts, not artifact bytes or package authenticity.
    pub fn with_annotations(
        mut self,
        method_json: Option<&str>,
        warning_json: Option<&str>,
        inventory_json: Option<&str>,
    ) -> Result<Self, EvidenceError> {
        require(
            !self.annotations_attached(),
            "annotations already supplied; cannot clear warning history",
        )?;
        require(
            method_json.is_some() == inventory_json.is_some(),
            "diagnostics and reference inventory must be paired",
        )?;
        let method = if let (Some(d), Some(i)) = (method_json, inventory_json) {
            diagnostics(&serde_json::from_str(d)?, &self, &serde_json::from_str(i)?)?
        } else {
            vec![]
        };
        let problem = if let Some(w) = warning_json {
            warnings(&serde_json::from_str(w)?, &self)?
        } else {
            vec![]
        };
        self.set_annotations(
            method,
            problem,
            method_json.is_some(),
            warning_json.is_some(),
        );
        Ok(self)
    }
}
