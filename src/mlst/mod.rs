//! Optional historical MLST for whole assemblies. Supplementary typing: it never sets or changes
//! a species, subspecies, lineage or genomovar.
//!
//! The engine is `terobacter-ungapped-mlst`, route D of the MLST feasibility study. It reproduces
//! the calls of `mlst` 2.32.2 with BLAST+ 2.16.0 on the development fixtures, but it is a new
//! engine and is reported as one. States, the profile-compatible candidate ST and the copy
//! diagnostics follow the frozen supplementary-MLST importer. No new-ST claim is made: an exact
//! profile that the scheme does not list is `unmatched_profile`.
//!
//! Pure: callers supply the package through [`PackageSource`](crate::reference::PackageSource)
//! and the assembly as parsed records.
mod engine;
mod metadata;
mod rules;
mod scheme;

pub use scheme::{Scheme, LIMITATION, LOCI, MANIFEST_PATH, SCHEME, SUBMISSION_CUTOFF};

use engine::revcomp;
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashSet};

pub const RECORD_SCHEMA: &str = "terobacter.mlst/1";
pub const ENGINE_NAME: &str = "terobacter-ungapped-mlst";
pub const ENGINE_VERSION: &str = "1";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MlstError {
    #[error("MLST package rejected: {0}")]
    Package(String),
    #[error("MLST input rejected: {0}")]
    Input(String),
    #[error("MLST result inconsistent: {0}")]
    Internal(String),
}

/// The engine's name, version and fixed matching profile.
pub fn engine() -> Value {
    json!({
        "name": ENGINE_NAME,
        "version": ENGINE_VERSION,
        "implementation": {"name": env!("CARGO_PKG_NAME"), "version": env!("CARGO_PKG_VERSION")},
        "method": "exact 32-mer seeds with ungapped X-drop extension",
        "profile": {
            "word_size": engine::WORD,
            "reward": engine::REWARD,
            "penalty": engine::PENALTY,
            "xdrop_raw": engine::XDROP,
            "min_identity_percent": rules::MIN_IDENTITY_PERCENT,
            "min_coverage_percent": rules::MIN_COVERAGE_PERCENT,
            "evalue": "1e-20",
            "ambiguous_bases": "mismatch",
        },
        "comparator": {
            "name": "mlst 2.32.2 with BLAST+ 2.16.0",
            "relation": "rules reproduced on development fixtures; comparator not executed",
        },
    })
}

/// One assembly record. `id` is the first word of its FASTA header.
pub struct Record<'a> {
    pub id: &'a str,
    pub seq: &'a [u8],
}

/// Type one whole assembly. `query_sha256` identifies the input bytes the records came from.
pub fn type_assembly(
    scheme: &Scheme,
    query_sha256: &str,
    records: &[Record],
) -> Result<Value, MlstError> {
    let mut ids = HashSet::new();
    for r in records {
        if r.id.is_empty() || !ids.insert(r.id) {
            return Err(MlstError::Input(format!(
                "record identifier {:?} is empty or repeated, so MLST locations would be ambiguous",
                r.id
            )));
        }
    }
    let contigs: Vec<Vec<u8>> = records
        .iter()
        .map(|r| {
            r.seq
                .iter()
                .map(|b| match b.to_ascii_uppercase() {
                    u @ (b'A' | b'C' | b'G' | b'T') => u,
                    _ => b'N',
                })
                .collect()
        })
        .collect();
    let lens: Vec<usize> = contigs.iter().map(Vec::len).collect();
    let call = rules::call(scheme, &lens, scheme.index.search(&contigs));
    interpret(scheme, query_sha256, records, &contigs, call)
}

/// The importer's states, candidate ST and copy diagnostics for one call.
fn interpret(
    scheme: &Scheme,
    query_sha256: &str,
    records: &[Record],
    contigs: &[Vec<u8>],
    call: rules::Call,
) -> Result<Value, MlstError> {
    let tokens = &call.tokens;
    let is_exact = |t: &str| {
        t.split(',')
            .all(|a| !a.is_empty() && a.bytes().all(|b| b.is_ascii_digit()))
    };
    let (state, st) = if tokens.iter().any(|t| t.contains(',')) {
        ("ambiguous_alleles", None)
    } else if tokens.iter().any(|t| t == "-") {
        ("incomplete_profile", None)
    } else if tokens.iter().any(|t| t.contains('~') || t.contains('?')) {
        ("inexact_alleles", None)
    } else {
        match scheme.profiles.get(tokens) {
            Some(st) => ("exact_profile_match", Some(st.clone())),
            None => ("unmatched_profile", None),
        }
    };
    if call.st != st.clone().unwrap_or_else(|| "-".into()) {
        return Err(MlstError::Internal(
            "comparator ST differs from the exact-profile state".into(),
        ));
    }

    let mut locations: Vec<Vec<Value>> = vec![Vec::new(); LOCI.len()];
    let mut places: Vec<BTreeSet<(usize, usize, usize)>> = vec![BTreeSet::new(); LOCI.len()];
    for hit in &call.exact {
        let allele = &scheme.alleles[hit.allele];
        let segment = &contigs[hit.contig][hit.start - 1..hit.end];
        let expected = if hit.minus {
            revcomp(&allele.seq)
        } else {
            allele.seq.clone()
        };
        if segment != expected.as_slice() {
            return Err(MlstError::Internal(
                "exact location does not match its allele sequence".into(),
            ));
        }
        locations[hit.locus].push(json!({
            "allele_id": allele.id, "record_id": records[hit.contig].id,
            "start_1based": hit.start, "end_1based_inclusive": hit.end,
            "strand": if hit.minus { "minus" } else { "plus" }}));
        places[hit.locus].insert((hit.contig, hit.start, hit.end));
    }

    let (mut repeated, mut discordant, mut same_sequence, mut unconfirmed) =
        (vec![], vec![], vec![], vec![]);
    let mut signature = Vec::new();
    let mut loci = Vec::new();
    for (li, (locus, token)) in LOCI.iter().zip(tokens).enumerate() {
        let numbers: Vec<&str> = token
            .split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .collect();
        let exact = is_exact(token);
        let ids: BTreeSet<&str> = if exact {
            numbers.iter().copied().collect()
        } else {
            BTreeSet::new()
        };
        signature.push((ids.len() == 1).then(|| ids.first().unwrap().to_string()));
        let locus_state = if token.contains(',') {
            "multiple_exact_alleles"
        } else if token == "-" {
            "no_reported_match"
        } else if token.starts_with('~') {
            "inexact_full_length"
        } else if token.ends_with('?') {
            "partial_match"
        } else {
            "single_exact_allele"
        };
        if token.contains(',') {
            let sequences: HashSet<&[u8]> = ids
                .iter()
                .map(|id| {
                    scheme.alleles[scheme.by_id[&(li, (*id).to_owned())]]
                        .seq
                        .as_slice()
                })
                .collect();
            if sequences.len() > 1 {
                discordant.push(*locus);
            } else if ids.len() > 1 {
                same_sequence.push(*locus);
            } else if places[li].len() > 1 {
                repeated.push(*locus);
            } else {
                unconfirmed.push(*locus);
            }
        }
        let mut located: Vec<&str> = locations[li]
            .iter()
            .map(|l| l["allele_id"].as_str().expect("allele id"))
            .collect();
        let mut reported = numbers.clone();
        located.sort_unstable();
        reported.sort_unstable();
        if exact && reported != located {
            return Err(MlstError::Internal(
                "exact location multiplicity disagrees with the allele token".into(),
            ));
        }
        loci.push(
            json!({"locus": locus, "raw_token": token, "state": locus_state,
            "reported_allele_ids": numbers, "reported_ids_are_exact": exact,
            "sequence_verified_exact_locations": locations[li]}),
        );
    }
    let candidate = if !repeated.is_empty() && unconfirmed.is_empty() {
        signature
            .iter()
            .cloned()
            .collect::<Option<Vec<String>>>()
            .and_then(|s| scheme.profiles.get(&s).cloned())
    } else {
        None
    };
    let reasons: Vec<&str> = [
        (&repeated, "repeated_identical_allele_copy"),
        (&discordant, "discordant_exact_allele_sequences"),
        (&same_sequence, "multiple_allele_ids_same_sequence"),
        (
            &unconfirmed,
            "repeated_hit_without_confirmed_multiple_locations",
        ),
    ]
    .into_iter()
    .filter(|(v, _)| !v.is_empty())
    .map(|(_, r)| r)
    .collect();
    let alleles: serde_json::Map<String, Value> = LOCI
        .iter()
        .zip(tokens)
        .map(|(l, t)| ((*l).to_owned(), t.clone().into()))
        .collect();
    Ok(json!({
        "schema": RECORD_SCHEMA,
        "status": "complete",
        "input_kind": "assembly",
        "query_sha256": query_sha256,
        "sequence_scope": "whole_assembly_as_supplied",
        "scheme": SCHEME,
        "state": state,
        "sequence_type": st,
        "alleles": alleles,
        "loci": loci,
        "profile_compatible_candidate_ST": candidate,
        "candidate_is_reported_ST": false,
        "copy_diagnostic_reasons": reasons,
        "repeated_same_allele_loci": repeated,
        "discordant_exact_allele_loci": discordant,
        "multiple_ids_same_sequence_loci": same_sequence,
        "unconfirmed_repeated_hit_loci": unconfirmed,
        "comparator_style": {"ST": call.st, "STATUS": call.status, "SCORE": call.score},
        "location_verification": "exact_locations_checked_against_the_hashed_query_sequence",
        "typing_role": "supplementary; does not establish genus, species, subspecies, lineage or genomovar",
        "taxonomic_assignment_performed": false,
        "scheme_routing": "explicitly_selected; applicability_not_established_by_ST",
        "engine": engine(),
        "database": scheme.database(),
    }))
}
