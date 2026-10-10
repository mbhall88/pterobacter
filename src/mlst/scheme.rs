//! The historical scheme package: a `manifest.json` whose SHA-256 the operator supplies, seven
//! locus FASTAs and the ST profile table, each listed in the manifest by SHA-256. The checks
//! follow the frozen supplementary-MLST importer, so a package it rejects is rejected here too.
use super::engine::{SeedIndex, MAX_ALLELES, MAX_ALLELE_LEN};
use super::MlstError;
use crate::evidence::sha256_hex;
use crate::reference::PackageSource;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Read;

pub const SCHEME: &str = "ecloacae";
pub const LOCI: [&str; 7] = ["dnaA", "fusA", "gyrB", "leuS", "pyrG", "rplB", "rpoB"];
pub const SUBMISSION_CUTOFF: &str = "2024-12-31";
pub const LIMITATION: &str = "Bundled MLST database: limited to PubMLST records submitted on or before 31 December 2024. Alleles, ST profiles and schemes submitted from 1 January 2025 onward are not included. Unmatched profiles may therefore correspond to STs described after this cutoff.";
pub const MANIFEST_PATH: &str = "manifest.json";
/// The largest file the loader reads; the real package's largest file is under 1 MB.
const MAX_FILE_BYTES: u64 = 64 << 20;

/// One allele, in database order: loci in scheme order, file order within a locus.
pub(crate) struct Allele {
    pub locus: usize,
    pub id: String,
    pub seq: Vec<u8>,
}

/// A verified scheme package, ready to type assemblies.
pub struct Scheme {
    pub(crate) manifest_sha256: String,
    pub(crate) manifest: Map<String, Value>,
    pub(crate) alleles: Vec<Allele>,
    /// `(locus, allele id)` to allele index.
    pub(crate) by_id: HashMap<(usize, String), usize>,
    /// Seven allele ids in locus order to ST.
    pub(crate) profiles: HashMap<Vec<String>, String>,
    pub(crate) index: SeedIndex,
}

fn invalid(message: impl Into<String>) -> MlstError {
    MlstError::Package(message.into())
}

fn read(source: &dyn PackageSource, path: &str) -> Result<Vec<u8>, MlstError> {
    let reader = source
        .open(path)
        .map_err(|e| invalid(format!("{path}: {e}")))?
        .ok_or_else(|| invalid(format!("{path} is missing")))?;
    let mut bytes = Vec::new();
    reader
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| invalid(format!("{path}: {e}")))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(invalid(format!("{path} is larger than the loader accepts")));
    }
    Ok(bytes)
}

fn text<'a>(manifest: &'a Map<String, Value>, key: &str) -> Result<&'a str, MlstError> {
    match manifest.get(key).and_then(Value::as_str) {
        Some(s) if !s.is_empty() && s == s.trim() => Ok(s),
        _ => Err(invalid(format!("manifest {key} must be non-empty text"))),
    }
}

fn is_sha256(s: &str) -> bool {
    crate::reference::is_sha256(s)
}

fn data_path(file: &str) -> String {
    format!("pubmlst/{SCHEME}/{file}")
}

/// Lines of a text file with LF or CRLF endings. A bare CR is refused.
fn lines(bytes: &[u8], path: &str) -> Result<Vec<String>, MlstError> {
    let text = std::str::from_utf8(bytes)
        .ok()
        .filter(|t| t.is_ascii())
        .ok_or_else(|| invalid(format!("{path} is not ASCII text")))?;
    let mut out = Vec::new();
    for line in text.split_terminator('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.contains('\r') {
            return Err(invalid(format!("{path} has a bare CR line ending")));
        }
        out.push(line.to_owned());
    }
    Ok(out)
}

/// `locus_N` records of A, C, G and T only, in file order.
fn alleles(bytes: &[u8], locus: &str, path: &str) -> Result<Vec<(String, Vec<u8>)>, MlstError> {
    if !bytes.starts_with(b">") {
        return Err(invalid(format!("{path} must start with a FASTA header")));
    }
    let mut records: Vec<(String, Vec<u8>)> = Vec::new();
    for line in lines(bytes, path)? {
        if let Some(header) = line.strip_prefix('>') {
            let name = header.split_whitespace().next().unwrap_or("");
            let id = name
                .strip_prefix(locus)
                .and_then(|rest| rest.strip_prefix('_'))
                .filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
                .ok_or_else(|| invalid(format!("{path}: unexpected allele record {name:?}")))?;
            records.push((id.to_owned(), Vec::new()));
        } else if !line.is_empty() {
            let record = records.last_mut().expect("first byte is a header");
            record
                .1
                .extend(line.bytes().map(|b| b.to_ascii_uppercase()));
        }
    }
    let mut seen = BTreeSet::new();
    for (id, seq) in &records {
        if seq.is_empty() || !seq.iter().all(|b| b"ACGT".contains(b)) {
            return Err(invalid(format!(
                "{path}: allele {id} is empty or not A, C, G and T"
            )));
        }
        if seq.len() >= MAX_ALLELE_LEN {
            return Err(invalid(format!("{path}: allele {id} is too long")));
        }
        if !seen.insert(id.as_str()) {
            return Err(invalid(format!("{path}: duplicate allele {id}")));
        }
    }
    Ok(records)
}

impl Scheme {
    /// Verify the package against the operator's manifest SHA-256 and build the seed index.
    pub fn verify(source: &dyn PackageSource, expected_sha256: &str) -> Result<Scheme, MlstError> {
        if !is_sha256(expected_sha256) {
            return Err(invalid("expected MLST manifest SHA-256 is malformed"));
        }
        let bytes = read(source, MANIFEST_PATH)?;
        if sha256_hex(&bytes) != expected_sha256 {
            return Err(invalid(
                "MLST manifest SHA-256 differs from the expected value",
            ));
        }
        let manifest: Map<String, Value> = serde_json::from_slice(&bytes)
            .map_err(|e| invalid(format!("manifest is not a JSON object: {e}")))?;
        if manifest.get("schema_version").and_then(Value::as_u64) != Some(1)
            || manifest.get("schema_version").map(Value::to_string) != Some("1".into())
            || manifest.get("scheme") != Some(&json!(SCHEME))
        {
            return Err(invalid("unsupported MLST package"));
        }
        if manifest.get("loci") != Some(&json!(LOCI))
            || manifest.get("submission_cutoff") != Some(&json!(SUBMISSION_CUTOFF))
        {
            return Err(invalid("unsupported locus order or submission cutoff"));
        }
        if manifest.get("limitation") != Some(&json!(LIMITATION)) {
            return Err(invalid("manifest lacks the required cutoff limitation"));
        }
        for key in [
            "package_id",
            "retrieved_utc",
            "sequence_state",
            "data_terms_url",
        ] {
            text(&manifest, key)?;
        }
        if manifest.get("complete_through_cutoff_independently_established") != Some(&json!(false))
        {
            return Err(invalid(
                "a package claiming exhaustive historical completeness is not supported",
            ));
        }
        let files: Vec<String> = LOCI
            .iter()
            .map(|l| format!("{l}.tfa"))
            .chain([format!("{SCHEME}.txt")])
            .map(|f| data_path(&f))
            .collect();
        let hashes = manifest
            .get("data_sha256")
            .and_then(Value::as_object)
            .filter(|h| {
                h.keys().collect::<BTreeSet<_>>() == files.iter().collect::<BTreeSet<_>>()
                    && h.values().all(|d| d.as_str().is_some_and(is_sha256))
            })
            .ok_or_else(|| invalid("unexpected package data inventory"))?;
        let mut data = BTreeMap::new();
        for path in &files {
            let bytes = read(source, path)?;
            if hashes[path].as_str() != Some(sha256_hex(&bytes).as_str()) {
                return Err(invalid(format!("{path} differs from its manifest SHA-256")));
            }
            data.insert(path.clone(), bytes);
        }

        let mut all = Vec::new();
        let mut by_id = HashMap::new();
        let mut counts = Map::new();
        for (li, locus) in LOCI.iter().enumerate() {
            let path = data_path(&format!("{locus}.tfa"));
            let records = alleles(&data[&path], locus, &path)?;
            counts.insert((*locus).into(), records.len().into());
            for (id, seq) in records {
                by_id.insert((li, id.clone()), all.len());
                all.push(Allele { locus: li, id, seq });
            }
        }
        if all.len() > MAX_ALLELES {
            return Err(invalid("too many alleles"));
        }
        let path = data_path(&format!("{SCHEME}.txt"));
        let rows = lines(&data[&path], &path)?;
        let header: Vec<&str> = rows.first().map_or(vec![], |h| h.split('\t').collect());
        let expected: Vec<&str> = ["ST"]
            .into_iter()
            .chain(LOCI)
            .chain(["clonal_complex"])
            .collect();
        if header != expected {
            return Err(invalid("unexpected MLST profile columns"));
        }
        let mut profiles = HashMap::new();
        let mut sts = BTreeSet::new();
        for row in &rows[1..] {
            let cols: Vec<&str> = row.split('\t').collect();
            if cols.len() != expected.len() {
                return Err(invalid("ragged profile row"));
            }
            let st = cols[0];
            let signature: Vec<String> = cols[1..=LOCI.len()].iter().map(|&s| s.into()).collect();
            if st.is_empty() || !st.bytes().all(|b| b.is_ascii_digit()) || !sts.insert(st) {
                return Err(invalid("invalid or duplicate ST"));
            }
            if signature
                .iter()
                .enumerate()
                .any(|(li, a)| !by_id.contains_key(&(li, a.clone())))
            {
                return Err(invalid(format!(
                    "profile ST{st} references an absent allele"
                )));
            }
            if profiles.insert(signature, st.to_owned()).is_some() {
                return Err(invalid("duplicate profile"));
            }
        }
        if profiles.is_empty()
            || manifest.get("profile_count").and_then(Value::as_u64) != Some(profiles.len() as u64)
            || manifest.get("allele_counts") != Some(&Value::Object(counts))
        {
            return Err(invalid("package record count mismatch"));
        }
        let index = SeedIndex::build(all.iter().map(|a| a.seq.clone()).collect());
        Ok(Scheme {
            manifest_sha256: expected_sha256.into(),
            manifest,
            alleles: all,
            by_id,
            profiles,
            index,
        })
    }

    /// Check that the manifest and every data file still have their verified bytes.
    pub fn recheck(&self, source: &dyn PackageSource) -> Result<(), MlstError> {
        if sha256_hex(&read(source, MANIFEST_PATH)?) != self.manifest_sha256 {
            return Err(invalid("MLST manifest changed during the run"));
        }
        for (path, digest) in self.manifest["data_sha256"].as_object().expect("verified") {
            if digest.as_str() != Some(sha256_hex(&read(source, path)?).as_str()) {
                return Err(invalid(format!("{path} changed during the run")));
            }
        }
        Ok(())
    }

    /// The SHA-256 of the manifest this scheme was verified against.
    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }

    /// Identity, cutoff, retrieval and licence provenance for reports.
    pub fn database(&self) -> Value {
        let m = &self.manifest;
        json!({
            "package_id": m["package_id"],
            "package_manifest_sha256": self.manifest_sha256,
            "scheme": SCHEME,
            "scheme_description": m.get("scheme_description"),
            "loci": LOCI,
            "submission_cutoff": SUBMISSION_CUTOFF,
            "retrieved_utc": m["retrieved_utc"],
            "sequence_state": m["sequence_state"],
            "complete_through_cutoff_independently_established": false,
            "post_cutoff_currency": "not_claimed",
            "limitation": LIMITATION,
            "data_terms_url": m["data_terms_url"],
            "data_sha256": m["data_sha256"],
            "allele_count": self.alleles.len(),
            "profile_count": self.profiles.len(),
        })
    }
}
