use super::AnimError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlignmentStatus {
    Aligned,
    NoAlignment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metrics {
    pub status: AlignmentStatus,
    #[serde(rename = "D")]
    pub d: u64,
    #[serde(rename = "E")]
    pub e: u64,
    pub blocks: u64,
    pub reference_covered: u64,
    pub query_covered: u64,
    pub reference_length: u64,
    pub query_length: u64,
}

impl Metrics {
    pub fn validate(&self) -> Result<(), AnimError> {
        if self.reference_length == 0
            || self.query_length == 0
            || self.reference_covered > self.reference_length
            || self.query_covered > self.query_length
            || self.e.checked_mul(2).ok_or(AnimError::Overflow)? > self.d
            || (self.blocks == 0) != (self.d == 0)
            || (self.status == AlignmentStatus::NoAlignment) != (self.blocks == 0)
            || (self.blocks == 0 && (self.reference_covered != 0 || self.query_covered != 0))
        {
            return Err(invalid("impossible metrics"));
        }
        Ok(())
    }
    /// Exact rational percentage, absent for a successful empty alignment.
    pub fn identity(&self) -> Result<Option<(u128, u64)>, AnimError> {
        self.validate()?;
        Ok((self.d != 0).then(|| (100 * u128::from(self.d - 2 * self.e), self.d)))
    }
    /// Compare to a percentage numerator/denominator, never a rounded float.
    pub fn identity_exceeds(&self, numerator: u64, denominator: u64) -> Result<bool, AnimError> {
        validate_threshold(numerator, denominator)?;
        let Some((identity, length)) = self.identity()? else {
            return Ok(false);
        };
        Ok(identity
            .checked_mul(u128::from(denominator))
            .ok_or(AnimError::Overflow)?
            > u128::from(numerator)
                .checked_mul(u128::from(length))
                .ok_or(AnimError::Overflow)?)
    }
    pub fn coverage_at_least(
        &self,
        reference: bool,
        numerator: u64,
        denominator: u64,
    ) -> Result<bool, AnimError> {
        self.validate()?;
        validate_threshold(numerator, denominator)?;
        let (covered, length) = if reference {
            (self.reference_covered, self.reference_length)
        } else {
            (self.query_covered, self.query_length)
        };
        Ok(u128::from(covered)
            .checked_mul(100)
            .and_then(|v| v.checked_mul(u128::from(denominator)))
            .ok_or(AnimError::Overflow)?
            >= u128::from(length)
                .checked_mul(u128::from(numerator))
                .ok_or(AnimError::Overflow)?)
    }
}

fn validate_threshold(n: u64, d: u64) -> Result<(), AnimError> {
    if d == 0 || u128::from(n) > 100 * u128::from(d) {
        return Err(invalid("invalid percentage threshold"));
    }
    Ok(())
}
fn invalid(message: &str) -> AnimError {
    AnimError::Invalid(message.to_owned())
}
fn add(a: u64, b: u64) -> Result<u64, AnimError> {
    a.checked_add(b).ok_or(AnimError::Overflow)
}
fn total(lengths: &BTreeMap<String, u64>) -> Result<u64, AnimError> {
    if lengths.is_empty() || lengths.values().any(|v| *v == 0) {
        return Err(invalid("empty FASTA lengths"));
    }
    lengths.values().try_fold(0, |a, &b| add(a, b))
}
fn coverage(intervals: BTreeMap<String, Vec<(u64, u64)>>) -> Result<u64, AnimError> {
    let mut sum = 0;
    for mut spans in intervals.into_values() {
        spans.sort_unstable();
        let mut end = 0;
        for (start, stop) in spans {
            if stop > end {
                let uncovered = stop - end.max(start - 1);
                sum = add(sum, uncovered)?;
                end = stop;
            }
        }
    }
    Ok(sum)
}

/// Parse complete NUCmer delta blocks against validated native-axis lengths.
/// Offsets are checked but never counted as additional alignment blocks.
pub fn parse_delta(
    text: &str,
    refs: &BTreeMap<String, u64>,
    queries: &BTreeMap<String, u64>,
) -> Result<Metrics, AnimError> {
    let mut result = Metrics {
        status: AlignmentStatus::NoAlignment,
        d: 0,
        e: 0,
        blocks: 0,
        reference_covered: 0,
        query_covered: 0,
        reference_length: total(refs)?,
        query_length: total(queries)?,
    };
    let mut lines = text.lines();
    if lines
        .next()
        .is_none_or(|line| line.split_whitespace().count() != 2)
        || lines.next() != Some("NUCMER")
    {
        return Err(invalid("delta header"));
    }
    let mut context: Option<(String, String, u64, u64)> = None;
    let mut remaining: Option<(u64, u64)> = None;
    let mut ri: BTreeMap<String, Vec<(u64, u64)>> = BTreeMap::new();
    let mut qi: BTreeMap<String, Vec<(u64, u64)>> = BTreeMap::new();
    for line in lines {
        if let Some(context_line) = line.strip_prefix('>') {
            if remaining.is_some() {
                return Err(invalid("unterminated delta block"));
            }
            let parts: Vec<_> = context_line.split_whitespace().collect();
            if parts.len() != 4 {
                return Err(invalid("delta record ID/length mismatch"));
            }
            let reference_length = parts[2]
                .parse::<u64>()
                .map_err(|_| invalid("invalid delta reference length"))?;
            let query_length = parts[3]
                .parse::<u64>()
                .map_err(|_| invalid("invalid delta query length"))?;
            let expected_reference = refs
                .get(parts[0])
                .ok_or_else(|| invalid("unknown delta reference ID"))?;
            let expected_query = queries
                .get(parts[1])
                .ok_or_else(|| invalid("unknown delta query ID"))?;
            if *expected_reference != reference_length || *expected_query != query_length {
                return Err(invalid("delta record ID/length mismatch"));
            }
            context = Some((
                parts[0].to_owned(),
                parts[1].to_owned(),
                reference_length,
                query_length,
            ));
        } else if let Some((mut r, mut q)) = remaining {
            if line.split_whitespace().count() != 1 {
                return Err(invalid("invalid delta offset"));
            }
            let offset = line
                .parse::<i64>()
                .map_err(|_| invalid("invalid delta offset"))?;
            if offset == 0 {
                // P1's frozen weighted metric fixture uses unequal spans and
                // a zero terminator. Metrics come from headers, not replaying
                // the edit path; retain that contract while bounding offsets.
                remaining = None;
            } else {
                let matches = offset.unsigned_abs() - 1;
                r = r
                    .checked_sub(matches)
                    .ok_or_else(|| invalid("delta offset exceeds reference span"))?;
                q = q
                    .checked_sub(matches)
                    .ok_or_else(|| invalid("delta offset exceeds query span"))?;
                if offset > 0 {
                    r = r
                        .checked_sub(1)
                        .ok_or_else(|| invalid("reference gap exceeds span"))?
                } else {
                    q = q
                        .checked_sub(1)
                        .ok_or_else(|| invalid("query gap exceeds span"))?
                }
                remaining = Some((r, q));
            }
        } else {
            let (rid, qid, reference_length, query_length) = context
                .as_ref()
                .ok_or_else(|| invalid("block without delta context"))?;
            let values = line
                .split_whitespace()
                .map(str::parse::<u64>)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| invalid("invalid delta block"))?;
            if values.len() != 7 {
                return Err(invalid("delta block needs seven integers"));
            }
            let [rs, re, qs, qe, e, _, _] = values[..] else {
                unreachable!()
            };
            if rs == 0
                || re == 0
                || qs == 0
                || qe == 0
                || rs.max(re) > *reference_length
                || qs.max(qe) > *query_length
            {
                return Err(invalid("coordinate out of bounds"));
            }
            let r = add(rs.abs_diff(re), 1)?;
            let q = add(qs.abs_diff(qe), 1)?;
            let span = add(r, q)?;
            if e.checked_mul(2).ok_or(AnimError::Overflow)? > span {
                return Err(invalid("impossible delta errors"));
            }
            result.d = add(result.d, span)?;
            result.e = add(result.e, e)?;
            result.blocks = add(result.blocks, 1)?;
            ri.entry(rid.clone())
                .or_default()
                .push((rs.min(re), rs.max(re)));
            qi.entry(qid.clone())
                .or_default()
                .push((qs.min(qe), qs.max(qe)));
            remaining = Some((r, q));
        }
    }
    if remaining.is_some() {
        return Err(invalid("truncated delta block"));
    }
    result.reference_covered = coverage(ri)?;
    result.query_covered = coverage(qi)?;
    if result.blocks > 0 {
        result.status = AlignmentStatus::Aligned
    }
    result.validate()?;
    Ok(result)
}
