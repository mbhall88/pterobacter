//! Whole-assembly command seam agreed in TERO-SPEC-r1.4.
use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
mod common;
use common::package::{FIXTURE_DIR, PINNED_MANIFEST_SHA256};

#[test]
fn invalid_input_retains_a_partial_report_and_never_assigns() {
    let dir = tempfile::tempdir().unwrap();
    let query = dir.path().join("bad.fa");
    std::fs::write(&query, b"@reads\nACGT\n+\n!!!!\n").unwrap();
    let output = cargo_bin_cmd!("tero")
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

#[test]
fn unknown_policy_is_an_explicit_unavailable_policy_failure() {
    let output = cargo_bin_cmd!("tero")
        .args([
            "--reference-package",
            FIXTURE_DIR,
            "--expected-sha256",
            PINNED_MANIFEST_SHA256,
            "--policy",
            "unavailable-policy",
            "missing.fa",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(5));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["execution"]["stage"], "policy");
    assert_eq!(report["policy"]["id"], "unavailable-policy");
    assert_eq!(report["assignment"]["species_id"], Value::Null);
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
    let output = cargo_bin_cmd!("tero")
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
        let output = cargo_bin_cmd!("tero")
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
    let output = cargo_bin_cmd!("tero")
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
    for name in ["tero", "terobacter"] {
        let mut cmd = match name {
            "tero" => cargo_bin_cmd!("tero"),
            _ => cargo_bin_cmd!("terobacter"),
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
