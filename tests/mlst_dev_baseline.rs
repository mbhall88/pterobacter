//! The MLST development baseline: the 52 valid synthetic fixtures of the MLST feasibility study,
//! typed against the historical scheme package and compared with the frozen comparator calls
//! and importer records in `tests/fixtures/mlst-dev/expected.json`.
//!
//! Neither the scheme package nor the fixtures are committed, so the test is ignored by default.
//! Run it with
//!
//! ```sh
//! TEROBACTER_MLST_PACKAGE=<package dir> TEROBACTER_MLST_SHA256=<manifest sha256> \
//! TEROBACTER_MLST_FIXTURES=<fixture dir> cargo test --release --test mlst_dev_baseline -- --ignored
//! ```
//!
//! `tests/fixtures/mlst-dev/README.md` records how the expected file and the fixtures are derived.
use serde_json::{json, Value};

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
use terobacter::cli::mlst::parse_records;

use terobacter::mlst::{type_assembly, Record, Scheme, LOCI};

const EXPECTED: &str = include_str!("fixtures/mlst-dev/expected.json");

struct Files(std::path::PathBuf);

impl terobacter::reference::PackageSource for Files {
    fn open(&self, path: &str) -> std::io::Result<Option<Box<dyn std::io::Read + '_>>> {
        match std::fs::File::open(self.0.join(path)) {
            Ok(file) => Ok(Some(Box::new(file))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn inventory(&self) -> std::io::Result<std::collections::BTreeSet<String>> {
        unimplemented!("the scheme loader reads listed files only")
    }
}

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run the development baseline"))
}

/// Exact locations sorted as the expected file sorts them.
fn exact_locations(record: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    for (li, locus) in record["loci"].as_array().unwrap().iter().enumerate() {
        for l in locus["sequence_verified_exact_locations"]
            .as_array()
            .unwrap()
        {
            let mut l = l.clone();
            l["locus"] = LOCI[li].into();
            out.push(l);
        }
    }
    out.sort_by_key(|l| {
        (
            LOCI.iter().position(|x| *x == l["locus"]).unwrap(),
            l["allele_id"].as_str().unwrap().parse::<u64>().unwrap(),
            l["record_id"].as_str().unwrap().to_owned(),
            l["start_1based"].as_u64().unwrap(),
            l["end_1based_inclusive"].as_u64().unwrap(),
            l["strand"].as_str().unwrap().to_owned(),
        )
    });
    out
}

#[test]
#[ignore = "needs the historical scheme package and the regenerated development fixtures"]
fn development_fixtures_match_the_pinned_baseline() {
    let expected: Value = serde_json::from_str(EXPECTED).unwrap();
    assert_eq!(expected["schema"], "terobacter.mlst-dev-baseline/1");
    let package = std::path::PathBuf::from(env("TEROBACTER_MLST_PACKAGE"));
    let scheme = Scheme::verify(&Files(package), &env("TEROBACTER_MLST_SHA256")).unwrap();
    let fixtures = std::path::PathBuf::from(env("TEROBACTER_MLST_FIXTURES"));
    let mut differences = Vec::new();
    let all = expected["fixtures"].as_object().unwrap();
    assert_eq!(all.len(), 52);
    for (name, want) in all {
        let bytes = std::fs::read(fixtures.join(format!("{name}.fna"))).unwrap();
        assert_eq!(
            sha256_hex(&bytes),
            want["fixture_sha256"].as_str().unwrap(),
            "{name}: fixture bytes differ from the frozen fixture"
        );
        let records = parse_records(&bytes).unwrap();
        let records: Vec<Record> = records.iter().map(|(id, seq)| Record { id, seq }).collect();
        let got = type_assembly(&scheme, &sha256_hex(&bytes), &records).unwrap();
        let observed = json!({
            "alleles": got["alleles"],
            "comparator_st": got["comparator_style"]["ST"],
            "comparator_status": got["comparator_style"]["STATUS"],
            "comparator_score": got["comparator_style"]["SCORE"],
            "state": got["state"],
            "sequence_type": got["sequence_type"],
            "profile_compatible_candidate_ST": got["profile_compatible_candidate_ST"],
            "copy_diagnostic_reasons": got["copy_diagnostic_reasons"],
            "exact_locations": exact_locations(&got),
        });
        for (key, value) in observed.as_object().unwrap() {
            if want[key] != *value {
                differences.push(format!("{name} {key}: want {} got {value}", want[key]));
            }
        }
        assert_eq!(got["database"]["submission_cutoff"], "2024-12-31");
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}
