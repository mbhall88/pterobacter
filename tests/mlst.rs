//! Historical MLST through the library seam: package verification, typing states and the
//! provenance every record carries.
mod common;
use common::mlst::{revcomp, SyntheticScheme, ALLELE_LEN};
use serde_json::{json, Value};
use terobacter::mlst::{type_assembly, MlstError, Record, Scheme, LIMITATION};

const QUERY_SHA256: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn verified(package: &SyntheticScheme) -> Scheme {
    Scheme::verify(package, &package.manifest_sha256()).unwrap()
}

fn type_one(package: &SyntheticScheme, pieces: &[Vec<u8>]) -> Value {
    let seq = package.assembly(pieces);
    type_assembly(
        &verified(package),
        QUERY_SHA256,
        &[Record {
            id: "contig1",
            seq: &seq,
        }],
    )
    .unwrap()
}

fn with_locus(package: &SyntheticScheme, locus: usize, piece: Option<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut pieces = package.st5_pieces();
    match piece {
        Some(p) => pieces[locus] = p,
        None => {
            pieces.remove(locus);
        }
    }
    pieces
}

#[test]
fn exact_alleles_on_both_strands_give_the_recorded_st() {
    let package = SyntheticScheme::new();
    let record = type_one(&package, &package.st5_pieces());
    assert_eq!(record["status"], "complete");
    assert_eq!(record["state"], "exact_profile_match");
    assert_eq!(record["sequence_type"], "5");
    assert_eq!(
        record["comparator_style"],
        json!({"ST": "5", "STATUS": "PERFECT", "SCORE": 100})
    );
    let strands: Vec<&str> = record["loci"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            assert_eq!(l["state"], "single_exact_allele");
            l["sequence_verified_exact_locations"][0]["strand"]
                .as_str()
                .unwrap()
        })
        .collect();
    assert_eq!(
        strands,
        ["plus", "minus", "plus", "minus", "plus", "minus", "plus"]
    );
    let first = &record["loci"][0]["sequence_verified_exact_locations"][0];
    assert_eq!(
        first,
        &json!({"allele_id": "1", "record_id": "contig1", "start_1based": 2001,
            "end_1based_inclusive": 2000 + ALLELE_LEN, "strand": "plus"})
    );
}

#[test]
fn an_exact_profile_the_scheme_lacks_is_unmatched_and_never_a_new_st() {
    let package = SyntheticScheme::new();
    let record = type_one(
        &package,
        &with_locus(&package, 1, Some(package.allele(1, 2))),
    );
    assert_eq!(record["state"], "unmatched_profile");
    assert_eq!(record["sequence_type"], Value::Null);
    assert_eq!(record["profile_compatible_candidate_ST"], Value::Null);
    assert_eq!(record["comparator_style"]["STATUS"], "NOVEL");
    assert_eq!(record["database"]["limitation"], LIMITATION);
}

#[test]
fn a_missing_locus_is_an_incomplete_profile() {
    let package = SyntheticScheme::new();
    let record = type_one(&package, &with_locus(&package, 3, None));
    assert_eq!(record["state"], "incomplete_profile");
    assert_eq!(record["alleles"]["leuS"], "-");
    assert_eq!(record["loci"][3]["state"], "no_reported_match");
    assert_eq!(record["comparator_style"]["STATUS"], "MISSING");
    assert_eq!(record["sequence_type"], Value::Null);
}

#[test]
fn full_length_mismatches_are_inexact_and_truncation_is_partial() {
    let package = SyntheticScheme::new();
    let mut inexact = package.allele(2, 1);
    inexact[60] = if inexact[60] == b'G' { b'T' } else { b'G' };
    let record = type_one(&package, &with_locus(&package, 2, Some(inexact)));
    assert_eq!(record["state"], "inexact_alleles");
    assert_eq!(record["alleles"]["gyrB"], "~1");
    assert_eq!(record["loci"][2]["state"], "inexact_full_length");
    assert_eq!(record["loci"][2]["reported_ids_are_exact"], false);

    let mut truncated = package.allele(4, 2);
    truncated.truncate(200);
    let record = type_one(
        &package,
        &with_locus(&package, 4, Some(revcomp(&truncated))),
    );
    assert_eq!(record["state"], "inexact_alleles");
    assert!(record["alleles"]["pyrG"].as_str().unwrap().ends_with('?'));
    assert_eq!(record["loci"][4]["state"], "partial_match");
    assert_eq!(record["sequence_type"], Value::Null);
}

#[test]
fn ambiguous_bases_are_mismatches() {
    let package = SyntheticScheme::new();
    let mut with_n = package.allele(5, 1);
    with_n[150] = b'N';
    let record = type_one(&package, &with_locus(&package, 5, Some(with_n)));
    assert_eq!(record["alleles"]["rplB"], "~2");
    let mut lower = package.st5_pieces();
    lower[0].make_ascii_lowercase();
    assert_eq!(type_one(&package, &lower)["sequence_type"], "5");
}

#[test]
fn a_repeated_identical_allele_is_ambiguous_with_a_candidate_st_only() {
    let package = SyntheticScheme::new();
    let mut pieces = package.st5_pieces();
    pieces.push(revcomp(&package.allele(0, 1)));
    let record = type_one(&package, &pieces);
    assert_eq!(record["state"], "ambiguous_alleles");
    assert_eq!(record["alleles"]["dnaA"], "1,1");
    assert_eq!(record["sequence_type"], Value::Null);
    assert_eq!(record["profile_compatible_candidate_ST"], "5");
    assert_eq!(record["candidate_is_reported_ST"], false);
    assert_eq!(
        record["copy_diagnostic_reasons"],
        json!(["repeated_identical_allele_copy"])
    );
    assert_eq!(
        record["loci"][0]["sequence_verified_exact_locations"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn two_different_exact_alleles_are_discordant_without_a_candidate() {
    let package = SyntheticScheme::new();
    let mut pieces = package.st5_pieces();
    pieces.push(package.allele(0, 3));
    let record = type_one(&package, &pieces);
    assert_eq!(record["state"], "ambiguous_alleles");
    assert_eq!(record["alleles"]["dnaA"], "1,3");
    assert_eq!(record["profile_compatible_candidate_ST"], Value::Null);
    assert_eq!(
        record["copy_diagnostic_reasons"],
        json!(["discordant_exact_allele_sequences"])
    );
    assert_eq!(record["comparator_style"]["STATUS"], "MIXED");
}

#[test]
fn alleles_split_across_records_are_found_on_each_record() {
    let package = SyntheticScheme::new();
    let pieces = package.st5_pieces();
    let seqs: Vec<Vec<u8>> = pieces
        .iter()
        .map(|p| package.assembly(std::slice::from_ref(p)))
        .collect();
    let ids: Vec<String> = (1..=seqs.len()).map(|i| format!("c{i}")).collect();
    let records: Vec<Record> = ids
        .iter()
        .zip(&seqs)
        .map(|(id, seq)| Record { id, seq })
        .collect();
    let record = type_assembly(&verified(&package), QUERY_SHA256, &records).unwrap();
    assert_eq!(record["sequence_type"], "5");
    assert_eq!(
        record["loci"][6]["sequence_verified_exact_locations"][0]["record_id"],
        "c7"
    );
}

#[test]
fn repeated_record_identifiers_fail_instead_of_guessing_locations() {
    let package = SyntheticScheme::new();
    let seq = package.assembly(&package.st5_pieces());
    let error = type_assembly(
        &verified(&package),
        QUERY_SHA256,
        &[
            Record {
                id: "same",
                seq: &seq,
            },
            Record {
                id: "same",
                seq: b"ACGT",
            },
        ],
    )
    .unwrap_err();
    assert!(matches!(error, MlstError::Input(_)), "{error}");
}

#[test]
fn records_carry_engine_identity_and_cutoff_provenance() {
    let package = SyntheticScheme::new();
    let record = type_one(&package, &package.st5_pieces());
    assert_eq!(record["engine"]["name"], "terobacter-ungapped-mlst");
    assert_eq!(record["engine"]["version"], "1");
    assert_eq!(record["engine"]["profile"]["word_size"], 32);
    assert_eq!(record["engine"]["profile"]["xdrop_raw"], 13);
    assert_eq!(record["taxonomic_assignment_performed"], false);
    let database = &record["database"];
    assert_eq!(database["package_id"], "synthetic-mlst-pre2025");
    assert_eq!(
        database["package_manifest_sha256"],
        package.manifest_sha256()
    );
    assert_eq!(database["submission_cutoff"], "2024-12-31");
    assert_eq!(database["retrieved_utc"], "2026-10-10T00:00:00+00:00");
    assert_eq!(database["post_cutoff_currency"], "not_claimed");
    assert_eq!(
        database["complete_through_cutoff_independently_established"],
        false
    );
    assert_eq!(
        database["data_terms_url"],
        "https://pubmlst.org/terms-conditions"
    );
    assert_eq!(database["data_sha256"], package.manifest()["data_sha256"]);
}

fn rejected(package: &SyntheticScheme) -> String {
    match Scheme::verify(package, &package.manifest_sha256()) {
        Err(MlstError::Package(message)) => message,
        Err(other) => panic!("unexpected error {other}"),
        Ok(_) => panic!("package accepted"),
    }
}

#[test]
fn the_manifest_must_have_the_expected_sha256() {
    let package = SyntheticScheme::new();
    let other = "1".repeat(64);
    assert!(matches!(
        Scheme::verify(&package, &other),
        Err(MlstError::Package(_))
    ));
    assert!(matches!(
        Scheme::verify(&package, "not-a-digest"),
        Err(MlstError::Package(_))
    ));
}

#[test]
fn altered_or_missing_data_is_rejected() {
    let mut package = SyntheticScheme::new();
    package
        .files
        .get_mut("pubmlst/ecloacae/gyrB.tfa")
        .unwrap()
        .push(b'\n');
    assert!(rejected(&package).contains("gyrB.tfa"));
    let mut package = SyntheticScheme::new();
    package.files.remove("pubmlst/ecloacae/ecloacae.txt");
    assert!(rejected(&package).contains("missing"));
}

#[test]
fn manifests_without_cutoff_provenance_or_claiming_completeness_are_rejected() {
    for (key, value) in [
        ("limitation", json!("A database.")),
        ("submission_cutoff", json!("2025-12-31")),
        ("retrieved_utc", json!("")),
        ("data_terms_url", Value::Null),
        (
            "complete_through_cutoff_independently_established",
            json!(true),
        ),
        ("schema_version", json!(1.0)),
        ("profile_count", json!(3)),
    ] {
        let mut package = SyntheticScheme::new();
        let mut manifest = package.manifest();
        manifest[key] = value;
        package.set_manifest(&manifest);
        rejected(&package);
    }
}

#[test]
fn malformed_alleles_and_profiles_are_rejected() {
    let mut package = SyntheticScheme::new();
    package.profiles.push(("7".into(), [9, 1, 1, 1, 1, 1, 1]));
    package.render();
    assert!(rejected(&package).contains("absent allele"));

    let mut package = SyntheticScheme::new();
    package.profiles.push(("7".into(), [1; 7]));
    package.render();
    assert!(rejected(&package).contains("duplicate profile"));

    let mut package = SyntheticScheme::new();
    package.alleles.get_mut(&(1, 2)).unwrap()[3] = b'N';
    package.render();
    assert!(rejected(&package).contains("not A, C, G and T"));
}

#[test]
fn a_recheck_rejects_a_package_changed_after_verification() {
    let mut package = SyntheticScheme::new();
    let scheme = verified(&package);
    scheme.recheck(&package).unwrap();
    package
        .files
        .get_mut("pubmlst/ecloacae/rpoB.tfa")
        .unwrap()
        .extend(b">rpoB_9\nACGT\n");
    assert!(matches!(
        scheme.recheck(&package),
        Err(MlstError::Package(_))
    ));
}

#[test]
fn overflowing_allele_ids_are_rejected_at_the_package_boundary() {
    let package = SyntheticScheme::new().with_dnaa_id("18446744073709551616");
    let error = Scheme::verify(&package, &package.manifest_sha256())
        .err()
        .unwrap();
    assert!(matches!(error, MlstError::Package(_)));
    assert!(error
        .to_string()
        .contains("exceeds the supported u64 range"));
}

#[test]
fn largest_supported_allele_id_can_be_typed_at_repeated_exact_locations() {
    let package = SyntheticScheme::new().with_dnaa_id("18446744073709551615");
    let mut pieces = package.st5_pieces();
    pieces.push(package.allele(0, 1));
    let record = type_one(&package, &pieces);
    assert_eq!(record["status"], "complete");
    assert_eq!(record["state"], "ambiguous_alleles");
    assert_eq!(
        record["loci"][0]["raw_token"],
        "18446744073709551615,18446744073709551615"
    );
}
