//! Version 2 source/currency metadata for compatible seven-locus snapshots and custom schemes.
use super::{MlstError, LOCI};
use serde_json::{json, Map, Value};
fn bad(message: &str) -> MlstError {
    MlstError::Package(message.into())
}
fn string<'a>(m: &'a Map<String, Value>, key: &str) -> Result<&'a str, MlstError> {
    m.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.trim() == *s)
        .ok_or_else(|| bad("new MLST metadata lacks required text"))
}
fn path(s: &str) -> bool {
    !s.is_empty()
        && s.split('/').all(|p| {
            !p.is_empty()
                && p != "."
                && p != ".."
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
}
fn url(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        s.starts_with("https://")
            && s.len() > 8
            && !s.contains('@')
            && !s.chars().any(char::is_whitespace)
    })
}
fn date(s: &str) -> bool {
    if s.len() != 10 || s.as_bytes()[4] != b'-' || s.as_bytes()[7] != b'-' {
        return false;
    }
    let parse = |a: usize, b: usize| s.get(a..b).and_then(|n| n.parse::<u32>().ok());
    let (Some(y), Some(m), Some(d)) = (parse(0, 4), parse(5, 7), parse(8, 10)) else {
        return false;
    };
    let max = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    y >= 1900 && d > 0 && d <= max
}
pub fn layout(m: &Map<String, Value>) -> Result<(String, String), MlstError> {
    if m.get("schema") != Some(&json!("pterobacter.mlst-scheme/2"))
        || m.get("schema_version").map(Value::to_string) != Some("2".into())
        || m.get("loci") != Some(&json!(LOCI))
    {
        return Err(bad("unsupported current/custom MLST schema or locus order"));
    }
    if [
        "submission_cutoff",
        "complete_through_cutoff_independently_established",
        "cutoff_evidence",
    ]
    .iter()
    .any(|k| m.contains_key(*k))
    {
        return Err(bad("schema 2 cannot inherit historical cutoff claims"));
    }
    let kind = string(m, "package_scope")?;
    if !["current_snapshot", "custom"].contains(&kind) {
        return Err(bad("unsupported MLST package scope"));
    }
    for k in [
        "scheme",
        "package_id",
        "sequence_state",
        "currency_statement",
    ] {
        string(m, k)?;
    }
    let time = string(m, "retrieved_utc")?;
    if !time.is_ascii()
        || time.len() != 20
        || !date(time.get(..10).unwrap_or(""))
        || &time[10..11] != "T"
        || &time[13..14] != ":"
        || &time[16..17] != ":"
        || !time.ends_with('Z')
        || time[11..13].parse::<u32>().map_or(true, |n| n > 23)
        || time[14..16].parse::<u32>().map_or(true, |n| n > 59)
        || time[17..19].parse::<u32>().map_or(true, |n| n > 59)
    {
        return Err(bad(
            "retrieved_utc must be a valid UTC timestamp YYYY-MM-DDTHH:MM:SSZ",
        ));
    }
    let source = m
        .get("source")
        .and_then(Value::as_object)
        .ok_or_else(|| bad("missing MLST source"))?;
    string(source, "name")?;
    if !source
        .get("urls")
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty() && a.iter().all(url))
    {
        return Err(bad("MLST source needs HTTPS provenance URLs"));
    }
    let rights = m
        .get("rights")
        .and_then(Value::as_object)
        .ok_or_else(|| bad("missing source rights metadata"))?;
    string(rights, "statement")?;
    string(rights, "access")?;
    if !rights.get("terms_url").is_some_and(url)
        || !["permitted", "restricted", "unknown"].contains(&string(rights, "redistribution")?)
    {
        return Err(bad("invalid source rights metadata"));
    }
    let coverage = m
        .get("submission_coverage")
        .and_then(Value::as_object)
        .ok_or_else(|| bad("submission coverage must be explicit"))?;
    match coverage.get("kind").and_then(Value::as_str) {
        Some("unknown") if coverage.get("through") == Some(&Value::Null) => {}
        Some("through_submission_date")
            if coverage
                .get("through")
                .and_then(Value::as_str)
                .is_some_and(date) => {}
        _ => {
            return Err(bad(
                "invalid submission coverage; retrieval does not establish currency",
            ))
        }
    }
    let root = string(m, "data_root")?;
    let profile = string(m, "profiles_file")?;
    if !path(root) || !path(profile) || profile.contains('/') {
        return Err(bad("unsafe MLST data layout"));
    }
    Ok((root.into(), profile.into()))
}
