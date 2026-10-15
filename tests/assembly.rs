//! Whole-assembly command seam agreed in TERO-SPEC-r1.4.
use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
mod common;
use common::package::{FIXTURE_DIR, PINNED_MANIFEST_SHA256};

fn skani_only(name: &str) -> assert_cmd::Command {
    let mut command = match name {
        "ptero" => cargo_bin_cmd!("ptero"),
        "pterobacter" => cargo_bin_cmd!("pterobacter"),
        _ => unreachable!(),
    };
    command.arg("--no-anim");
    command
}

#[test]
fn invalid_input_retains_a_partial_report_and_never_assigns() {
    let dir = tempfile::tempdir().unwrap();
    let query = dir.path().join("bad.fa");
    std::fs::write(&query, b"@reads\nACGT\n+\n!!!!\n").unwrap();
    let output = skani_only("ptero")
        .args([
            "--reference-package",
            FIXTURE_DIR,
            "--expected-sha256",
            PINNED_MANIFEST_SHA256,
        ])
        .arg(&query)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["execution"]["status"], "incomplete");
    assert_eq!(report["execution"]["stage"], "input");
    assert_eq!(report["assignment"]["species_id"], Value::Null);
    assert_eq!(
        report["input"]["query_sha256"],
        common::package::sha256_hex(&std::fs::read(query).unwrap())
    );
}

use common::assembly::native_package;

#[test]
fn native_whole_assembly_retains_plasmids_and_short_records_and_assigns_v1() {
    let (dir, pin, seq) = native_package();
    let query = dir.path().join("query.fa");
    let bytes = [
        b">draft chromosome\n".as_slice(),
        &seq,
        b"\n>plasmid extra\nACGTN\n",
    ]
    .concat();
    std::fs::write(&query, &bytes).unwrap();
    let output = skani_only("ptero")
        .arg("--reference-package")
        .arg(dir.path().join("package"))
        .args(["--expected-sha256", &pin])
        .arg(&query)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["execution"]["status"], "complete");
    assert_eq!(report["assignment"]["species_id"], "SYN-SP01", "{report}");
    assert_eq!(report["assignment"]["subspecies_id"], "SYN-SS01");
    assert_eq!(report["input"]["total_bases"], 200_008);
    assert_eq!(report["input"]["records"][1]["header"], "plasmid extra");
    assert_eq!(report["input"]["records"][1]["length"], 5);
    assert_eq!(report["measurement"]["sketched_bases"], 200_003);
    assert_eq!(
        report["measurement"]["engine_profile"],
        "skani-0.3.2-avx2-compatible-dna-v1"
    );
    assert_eq!(
        report["input"]["query_sha256"],
        common::package::sha256_hex(&bytes)
    );
    assert_eq!(
        report["reference_package"]["package"]["manifest_sha256"],
        pin
    );
    assert_eq!(report["mlst"]["status"], "not_requested");
}

#[test]
fn empty_malformed_and_non_fasta_inputs_fail_with_null_calls() {
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes) in [
        ("empty", b"".as_slice()),
        ("text", b"plain text\n"),
        ("no_sequence", b">empty\n"),
        ("no_identifier", b">\nACGT\n"),
        ("invalid_base", b">bad\nACGT!\n"),
        ("truncated_fastq", b"@read\nACGT\n+\n"),
    ] {
        let query = dir.path().join(name);
        std::fs::write(&query, bytes).unwrap();
        let output = skani_only("ptero")
            .args([
                "--reference-package",
                FIXTURE_DIR,
                "--expected-sha256",
                PINNED_MANIFEST_SHA256,
            ])
            .arg(query)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(3), "{name}");
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["execution"]["status"], "incomplete");
        assert_eq!(
            report["input"]["query_sha256"],
            common::package::sha256_hex(bytes)
        );
        assert_eq!(report["assignment"]["species_id"], Value::Null);
    }
}

#[test]
fn changed_package_is_rejected_before_any_assignment() {
    let (dir, pin, seq) = native_package();
    let query = dir.path().join("query.fa");
    std::fs::write(&query, [b">query\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    std::fs::write(
        dir.path()
            .join("package/databases/primary_whole/markers.bin"),
        b"corrupt",
    )
    .unwrap();
    let output = skani_only("ptero")
        .arg("--reference-package")
        .arg(dir.path().join("package"))
        .args(["--expected-sha256", &pin])
        .arg(query)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(4));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["execution"]["stage"], "package");
    assert_eq!(report["assignment"]["species_id"], Value::Null);
}

#[test]
fn complete_no_estimate_is_withheld_and_batch_failure_does_not_change_other_queries() {
    let (dir, pin, seq) = native_package();
    let query = dir.path().join("query.fa");
    let short = dir.path().join("short.fa");
    let bad = dir.path().join("bad.fa");
    std::fs::write(&query, [b">query\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    std::fs::write(&short, b">short\nACGT\n").unwrap();
    std::fs::write(&bad, b">bad\n!\n").unwrap();
    let mut observed = Vec::new();
    for name in ["ptero", "pterobacter"] {
        let mut cmd = match name {
            "ptero" => skani_only("ptero"),
            _ => skani_only("pterobacter"),
        };
        let output = cmd
            .arg("--reference-package")
            .arg(dir.path().join("package"))
            .args(["--expected-sha256", &pin, "--threads", "2"])
            .args([&query, &short, &bad])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(3));
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report[0]["execution"]["status"], "complete");
        assert_eq!(report[0]["assignment"]["species_id"], "SYN-SP01");
        assert_eq!(report[1]["execution"]["status"], "complete");
        assert_eq!(report[1]["assignment"]["species_id"], Value::Null);
        assert_eq!(
            report[1]["evidence"]["evidence_status"],
            "no_estimate_returned"
        );
        assert_eq!(
            report[1]["measurement"]["not_returned_reference_files"]
                .as_array()
                .unwrap()
                .len(),
            5
        );
        assert_eq!(report[2]["execution"]["status"], "incomplete");
        observed.push(report);
    }
    assert_eq!(observed[0], observed[1]);
}

/// A query that classifies as SYN-SP01 and carries the synthetic scheme's ST5 alleles.
fn mlst_query(dir: &std::path::Path, seq: &[u8], second_id: &str) -> std::path::PathBuf {
    let scheme = common::mlst::SyntheticScheme::new();
    scheme.write_to(&dir.join("mlst"));
    let query = dir.join("query-mlst.fa");
    let typing = scheme.assembly(&scheme.st5_pieces());
    let bytes = [
        b">chromosome\n".as_slice(),
        seq,
        format!("\n>{second_id} typing region\n").as_bytes(),
        &typing,
        b"\n",
    ]
    .concat();
    std::fs::write(&query, bytes).unwrap();
    query
}

fn run_with_mlst(
    dir: &std::path::Path,
    pin: &str,
    mlst_sha256: &str,
    queries: &[&std::path::Path],
) -> (Option<i32>, Value) {
    let output = skani_only("ptero")
        .arg("--reference-package")
        .arg(dir.join("package"))
        .args(["--expected-sha256", pin])
        .arg("--mlst-package")
        .arg(dir.join("mlst"))
        .args(["--mlst-expected-sha256", mlst_sha256])
        .args(queries)
        .output()
        .unwrap();
    let report = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&output.stderr)));
    (output.status.code(), report)
}

#[test]
fn requested_mlst_is_reported_beside_an_unchanged_classification() {
    let (dir, pin, seq) = native_package();
    let query = mlst_query(dir.path(), &seq, "typing");
    let sha256 = common::mlst::SyntheticScheme::new().manifest_sha256();
    let (code, report) = run_with_mlst(dir.path(), &pin, &sha256, &[&query]);
    assert_eq!(code, Some(0), "{report}");
    assert_eq!(report["execution"]["status"], "complete");
    assert_eq!(report["assignment"]["species_id"], "SYN-SP01");
    let mlst = &report["mlst"];
    assert_eq!(mlst["requested"], true);
    assert_eq!(mlst["status"], "complete");
    assert_eq!(mlst["state"], "exact_profile_match");
    assert_eq!(mlst["sequence_type"], "5");
    assert_eq!(mlst["query_sha256"], report["input"]["query_sha256"]);
    assert_eq!(
        mlst["loci"][0]["sequence_verified_exact_locations"][0]["record_id"],
        "typing"
    );
    assert_eq!(mlst["engine"]["name"], "terobacter-ungapped-mlst");
    assert_eq!(mlst["database"]["submission_cutoff"], "2024-12-31");
    assert_eq!(mlst["database"]["package_manifest_sha256"], sha256);
    assert_eq!(mlst["taxonomic_assignment_performed"], false);
}

#[test]
fn requested_mlst_that_fails_is_explicit_and_keeps_the_classification() {
    let (dir, pin, seq) = native_package();
    let query = mlst_query(dir.path(), &seq, "typing");
    let duplicate = mlst_query(dir.path(), &seq, "chromosome");
    let wrong = "1".repeat(64);
    let (code, report) = run_with_mlst(dir.path(), &pin, &wrong, &[&query]);
    assert_eq!(code, Some(6));
    assert_eq!(report["execution"]["status"], "complete");
    assert_eq!(report["assignment"]["species_id"], "SYN-SP01");
    assert_eq!(report["mlst"]["status"], "failed");
    assert_eq!(report["mlst"]["stage"], "package");
    assert_eq!(report["mlst"]["sequence_type"], Value::Null);

    let sha256 = common::mlst::SyntheticScheme::new().manifest_sha256();
    let (code, report) = run_with_mlst(dir.path(), &pin, &sha256, &[&duplicate]);
    assert_eq!(code, Some(6));
    assert_eq!(report["assignment"]["species_id"], "SYN-SP01");
    assert_eq!(report["mlst"]["status"], "failed");
    assert_eq!(report["mlst"]["stage"], "input");

    let bad = dir.path().join("bad.fa");
    std::fs::write(&bad, b">bad\n!\n").unwrap();
    let (code, report) = run_with_mlst(dir.path(), &pin, &sha256, &[&bad]);
    assert_eq!(code, Some(3));
    assert_eq!(report["execution"]["status"], "incomplete");
    assert_eq!(report["mlst"]["status"], "failed");
    assert_eq!(report["mlst"]["stage"], "input");
}

#[test]
fn an_mlst_package_needs_its_expected_sha256() {
    let output = skani_only("ptero")
        .args([
            "--reference-package",
            FIXTURE_DIR,
            "--expected-sha256",
            PINNED_MANIFEST_SHA256,
            "--mlst-package",
            "mlst",
            "query.fa",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

#[test]
fn optional_mlst_success_and_overflow_failure_preserve_v1_and_native_v2_calls() {
    let (dir, pin, seq) = native_package();
    let query = mlst_query(dir.path(), &seq, "typing");
    // Include two exact dnaA copies: the accepted malformed package previously panicked here.
    let mut bytes = std::fs::read(&query).unwrap();
    bytes.extend(b">second-copy\n");
    bytes.extend(common::mlst::SyntheticScheme::new().allele(0, 1));
    bytes.push(b'\n');
    std::fs::write(&query, bytes).unwrap();
    for policy in ["conservative-assembly-v1", "conservative-assembly-anim-v2"] {
        for id in ["1", "18446744073709551616"] {
            let scheme = common::mlst::SyntheticScheme::new().with_dnaa_id(id);
            scheme.write_to(&dir.path().join("mlst"));
            let output = cargo_bin_cmd!("ptero")
                .arg("--reference-package")
                .arg(dir.path().join("package"))
                .args(["--expected-sha256", &pin])
                .args(if policy == "conservative-assembly-v1" {
                    vec!["--no-anim"]
                } else {
                    vec![]
                })
                .arg("--mlst-package")
                .arg(dir.path().join("mlst"))
                .args(["--mlst-expected-sha256", &scheme.manifest_sha256()])
                .arg(&query)
                .output()
                .unwrap();
            let report: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
                panic!(
                    "lost partial JSON: {}",
                    String::from_utf8_lossy(&output.stderr)
                )
            });
            assert_eq!(report["execution"]["status"], "complete", "{report}");
            let species = if policy == "conservative-assembly-anim-v2" {
                &report["species_id"]
            } else {
                &report["assignment"]["species_id"]
            };
            assert_eq!(species, "SYN-SP01", "{report}");
            if policy == "conservative-assembly-anim-v2" {
                assert_eq!(report["policy_id"], policy);
                assert_eq!(report["rescue_status"], "not_triggered");
            }
            if id == "1" {
                assert_eq!(output.status.code(), Some(0));
                assert_eq!(report["mlst"]["status"], "complete");
                assert_eq!(report["mlst"]["state"], "ambiguous_alleles");
            } else {
                assert_eq!(output.status.code(), Some(6));
                assert_eq!(report["mlst"]["status"], "failed");
                assert_eq!(report["mlst"]["stage"], "package");
                assert_eq!(report["mlst"]["sequence_type"], Value::Null);
                assert!(report["mlst"]["error"]
                    .as_str()
                    .unwrap()
                    .contains("exceeds the supported u64 range"));
            }
        }
    }
}
