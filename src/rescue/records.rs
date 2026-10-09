//! Saved reciprocal ANIm pair records: exact integers in biological roles, one record per line
//! or one per file.
//!
//! This reads the subset of the per-pair record (`terobacter-anim-pair-v1`) that a decision
//! needs. Receipts and native artifacts are the producer's concern; fields not listed here are
//! ignored. Malformed metrics are input errors, never a missing or negative measurement.
use std::cmp::Ordering;

use serde::Deserialize;

use crate::evidence::{invalid, is_sha256, EvidenceError};

use super::thresholds::Exact;

pub(crate) const PAIR_SCHEMA: &str = "terobacter-anim-pair-v1";
const WHOLE_ASSEMBLY: &str = "whole_assembly";
const PRIMARY_COMPONENT: &str = "primary_whole";

/// One ANIm orientation, as measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Orientation {
    /// D and E from the retained blocks, with union-covered bases for each biological role.
    Aligned {
        d: u64,
        e: u64,
        query_covered: u64,
        query_length: u64,
        reference_covered: u64,
        reference_length: u64,
    },
    /// The tool chain succeeded and retained no block: a measured, non-supporting result.
    NoAlignment,
    /// The orientation was attempted and did not produce a valid measurement.
    Failed,
}

fn at_least_80(covered: u64, length: u64) -> bool {
    100 * u128::from(covered) >= 80 * u128::from(length)
}

impl Orientation {
    pub(crate) fn finished(&self) -> bool {
        !matches!(self, Orientation::Failed)
    }

    /// Identity as the exact rational 100 (D − 2E) / D.
    fn identity(&self) -> Option<(u128, u128)> {
        match *self {
            Orientation::Aligned { d, e, .. } => Some((100 * u128::from(d - 2 * e), u128::from(d))),
            _ => None,
        }
    }

    fn identity_vs(&self, threshold: &Exact) -> Option<Ordering> {
        let (num, den) = self.identity()?;
        let order = threshold
            .cmp_ratio(num, den)
            .expect("aligned metrics have a nonzero denominator");
        Some(order.reverse())
    }

    fn reference_af_80(&self) -> bool {
        matches!(*self, Orientation::Aligned { reference_covered, reference_length, .. }
            if at_least_80(reference_covered, reference_length))
    }

    fn both_af_80(&self) -> bool {
        matches!(*self, Orientation::Aligned { query_covered, query_length, .. }
            if at_least_80(query_covered, query_length))
            && self.reference_af_80()
    }

    /// The old method gate: identity ≥ T and both AFs ≥ 80.
    pub(crate) fn gate(&self, threshold: &Exact) -> bool {
        self.both_af_80()
            && self
                .identity_vs(threshold)
                .is_some_and(|o| o != Ordering::Less)
    }

    /// The rescue gate: identity strictly > T and both AFs ≥ 80.
    pub(crate) fn strict(&self, threshold: &Exact) -> bool {
        self.both_af_80() && self.identity_vs(threshold) == Some(Ordering::Greater)
    }

    /// Support for another parent: identity ≥ the conflict ANI and reference AF ≥ 80.
    pub(crate) fn challenges(&self, conflict: &Exact) -> bool {
        self.reference_af_80()
            && self
                .identity_vs(conflict)
                .is_some_and(|o| o != Ordering::Less)
    }

    /// Identity for display; `None` unless aligned.
    pub(crate) fn identity_percent(&self) -> Option<f64> {
        self.identity().map(|(n, d)| n as f64 / d as f64)
    }

    /// Query and reference AF for display; zero with no alignment, `None` when failed.
    pub(crate) fn af_percents(&self) -> Option<[f64; 2]> {
        let percent = |covered: u64, length: u64| 100.0 * covered as f64 / length as f64;
        match *self {
            Orientation::Aligned {
                query_covered,
                query_length,
                reference_covered,
                reference_length,
                ..
            } => Some([
                percent(query_covered, query_length),
                percent(reference_covered, reference_length),
            ]),
            Orientation::NoAlignment => Some([0.0, 0.0]),
            Orientation::Failed => None,
        }
    }

    pub(crate) fn to_json(self) -> serde_json::Value {
        let [af_query, af_reference] = self.af_percents().unwrap_or([0.0, 0.0]);
        match self {
            Orientation::Aligned {
                d,
                e,
                query_covered,
                query_length,
                reference_covered,
                reference_length,
            } => serde_json::json!({"status": "aligned",
                "identity_percent": self.identity_percent(),
                "af_query_percent": af_query, "af_reference_percent": af_reference,
                "D": d, "E": e, "query_covered": query_covered, "query_length": query_length,
                "reference_covered": reference_covered, "reference_length": reference_length}),
            Orientation::NoAlignment => serde_json::json!({"status": "no_alignment",
                "identity_percent": null, "af_query_percent": af_query,
                "af_reference_percent": af_reference}),
            Orientation::Failed => serde_json::json!({"status": "failed"}),
        }
    }
}

/// One saved pair record, structurally valid but not yet bound to a decision's inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PairRecord {
    pub query_sha256: Option<String>,
    pub query_bases: Option<u64>,
    pub reference_id: Option<String>,
    pub reference_sha256: Option<String>,
    pub reference_bases: Option<u64>,
    pub profile: String,
    /// Native reference R, native query Q.
    pub qr: Option<Orientation>,
    /// Native reference Q, native query R.
    pub rq: Option<Orientation>,
}

/// Saved ANIm pair records for one or more queries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnimRecords {
    pub(crate) records: Vec<PairRecord>,
}

#[derive(Deserialize)]
struct RawRecord {
    schema: String,
    status: String,
    query: Option<RawInput>,
    reference: Option<RawInput>,
    method: RawMethod,
    orientations: RawOrientations,
}

#[derive(Deserialize)]
struct RawInput {
    id: Option<String>,
    sha256: String,
    total_bases: u64,
    scope: String,
    component: String,
}

#[derive(Deserialize)]
struct RawMethod {
    profile: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOrientations {
    #[serde(rename = "QR")]
    qr: Option<RawOrientation>,
    #[serde(rename = "RQ")]
    rq: Option<RawOrientation>,
}

#[derive(Deserialize)]
struct RawOrientation {
    method_profile: String,
    native_reference_id: String,
    native_query_id: String,
    status: String,
    metrics: Option<RawMetrics>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMetrics {
    status: String,
    #[serde(rename = "D")]
    d: u64,
    #[serde(rename = "E")]
    e: u64,
    blocks: u64,
    reference_covered: u64,
    query_covered: u64,
    reference_length: u64,
    query_length: u64,
}

/// The pair's inputs: method profile, base counts and the native (reference, query) IDs this
/// orientation must name. Metrics are in biological roles whatever the native axes.
struct Expected<'a> {
    profile: &'a str,
    query_bases: Option<u64>,
    reference_bases: Option<u64>,
    native: Option<(&'a str, &'a str)>,
}

fn orientation(raw: RawOrientation, expected: &Expected<'_>) -> Result<Orientation, EvidenceError> {
    let Expected {
        profile,
        query_bases,
        reference_bases,
        native,
    } = *expected;
    if raw.method_profile != profile {
        return invalid("an orientation's method profile differs from its pair");
    }
    if native.is_some_and(|(r, q)| raw.native_reference_id != r || raw.native_query_id != q) {
        return invalid("an orientation's native reference and query do not match its direction");
    }
    let metrics = |status: &str| -> Result<RawMetrics, EvidenceError> {
        let Some(m) = raw.metrics else {
            return invalid(format!("{status} orientation without metrics"));
        };
        if m.status != status
            || m.query_length == 0
            || m.reference_length == 0
            || query_bases.is_some_and(|q| q != m.query_length)
            || reference_bases.is_some_and(|r| r != m.reference_length)
        {
            return invalid("orientation metrics disagree with the pair's inputs");
        }
        Ok(m)
    };
    match raw.status.as_str() {
        "aligned" => {
            let m = metrics("aligned")?;
            if m.blocks == 0
                || m.d == 0
                || m.e.checked_mul(2).is_none_or(|e2| e2 > m.d)
                || m.query_covered == 0
                || m.reference_covered == 0
                || m.query_covered > m.query_length
                || m.reference_covered > m.reference_length
            {
                return invalid("impossible aligned ANIm metrics");
            }
            Ok(Orientation::Aligned {
                d: m.d,
                e: m.e,
                query_covered: m.query_covered,
                query_length: m.query_length,
                reference_covered: m.reference_covered,
                reference_length: m.reference_length,
            })
        }
        "no_alignment" => {
            let m = metrics("no_alignment")?;
            if m.blocks != 0
                || m.d != 0
                || m.e != 0
                || m.query_covered != 0
                || m.reference_covered != 0
            {
                return invalid("a no-alignment orientation must have zero blocks and coverage");
            }
            Ok(Orientation::NoAlignment)
        }
        "error" | "not_run" => Ok(Orientation::Failed),
        other => invalid(format!("unknown ANIm orientation status {other}")),
    }
}

fn input(raw: Option<RawInput>, role: &str) -> Result<Option<RawInput>, EvidenceError> {
    if let Some(i) = &raw {
        if !is_sha256(&i.sha256) || i.total_bases == 0 || i.scope != WHOLE_ASSEMBLY {
            return invalid(format!("invalid {role} binding in ANIm record"));
        }
    }
    Ok(raw)
}

impl AnimRecords {
    /// Parse JSON Lines, one pair record per non-blank line.
    pub fn from_jsonl(text: &str) -> Result<Self, EvidenceError> {
        let mut records = Vec::new();
        for (n, line) in text
            .lines()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty())
        {
            let at = |e: EvidenceError| {
                let detail = match e {
                    EvidenceError::Invalid(message) => message,
                    other => other.to_string(),
                };
                EvidenceError::Invalid(format!("ANIm record line {}: {detail}", n + 1))
            };
            let raw: RawRecord = serde_json::from_str(line).map_err(|e| at(e.into()))?;
            records.push(Self::record(raw).map_err(at)?);
        }
        Ok(AnimRecords { records })
    }

    fn record(raw: RawRecord) -> Result<PairRecord, EvidenceError> {
        if raw.schema != PAIR_SCHEMA {
            return invalid(format!(
                "expected ANIm schema {PAIR_SCHEMA}, found {}",
                raw.schema
            ));
        }
        if raw.method.profile.trim().is_empty() {
            return invalid("ANIm method profile required");
        }
        let query = input(raw.query, "query")?;
        let reference = input(raw.reference, "reference")?;
        // Both IDs are needed to check that each orientation's native roles match its direction.
        let blank = |i: &RawInput| i.id.as_deref().is_none_or(|id| id.trim().is_empty());
        if query.as_ref().is_some_and(blank) {
            return invalid("ANIm query ID required");
        }
        if reference.as_ref().is_some_and(blank) {
            return invalid("ANIm reference ID required");
        }
        let (qb, rb) = (
            query.as_ref().map(|q| q.total_bases),
            reference.as_ref().map(|r| r.total_bases),
        );
        if reference
            .as_ref()
            .is_some_and(|r| r.component != PRIMARY_COMPONENT)
        {
            return invalid("ANIm reference is not from the primary whole-assembly component");
        }
        let profile = raw.method.profile;
        // QR aligns the query to the reference as native reference; RQ swaps the axes.
        let native_ids = query
            .as_ref()
            .and_then(|q| q.id.as_deref())
            .zip(reference.as_ref().and_then(|r| r.id.as_deref()))
            .map(|(q, r)| (r, q));
        let expected = |native| Expected {
            profile: &profile,
            query_bases: qb,
            reference_bases: rb,
            native,
        };
        let qr = raw
            .orientations
            .qr
            .map(|o| orientation(o, &expected(native_ids)))
            .transpose()?;
        let rq = raw
            .orientations
            .rq
            .map(|o| orientation(o, &expected(native_ids.map(|(r, q)| (q, r)))))
            .transpose()?;
        let finished = [qr, rq].iter().all(|o| o.is_some_and(|o| o.finished()));
        if (raw.status == "complete") != finished {
            return invalid("pair status disagrees with its orientations");
        }
        Ok(PairRecord {
            query_sha256: query.as_ref().map(|q| q.sha256.clone()),
            query_bases: qb,
            reference_id: reference.as_ref().and_then(|r| r.id.clone()),
            reference_sha256: reference.as_ref().map(|r| r.sha256.clone()),
            reference_bases: rb,
            profile,
            qr,
            rq,
        })
    }

    /// Parse one pair record, as the per-pair operation writes it.
    pub fn from_json(text: &str) -> Result<Self, EvidenceError> {
        let raw: RawRecord = serde_json::from_str(text)?;
        Ok(AnimRecords {
            records: vec![Self::record(raw)?],
        })
    }

    /// Add the records of another file.
    pub fn extend(&mut self, other: AnimRecords) {
        self.records.extend(other.records);
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}
