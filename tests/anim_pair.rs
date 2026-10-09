#![cfg(feature = "dev")]
use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
use std::path::{Path, PathBuf};
use terobacter::anim::{hash_file, verify_pair, ExpectedPair, PairRecord};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/anim")
        .join(name)
}
fn pair(query: &Path, reference: &Path, output: &Path) -> std::process::Output {
    let result = cargo_bin_cmd!("terobacter-dev")
        .args(["anim-pair", "--query-id", "Q", "--reference-id", "R"])
        .arg("--query")
        .arg(query)
        .arg("--reference")
        .arg(reference)
        .arg("--output")
        .arg(output)
        .output()
        .unwrap();
    if !result.status.success() {
        eprintln!("pair stdout: {}", String::from_utf8_lossy(&result.stdout));
        for direction in ["QR", "RQ"] {
            if let Ok(stderr) =
                std::fs::read_to_string(output.join(direction).join("worker.stderr"))
            {
                eprintln!("{direction} worker stderr: {stderr}");
            }
        }
    }
    result
}
fn expected_pair(query_sha256: &str, reference_sha256: &str) -> ExpectedPair {
    ExpectedPair {
        query_id: "Q".into(),
        query_sha256: query_sha256.into(),
        reference_id: "R".into(),
        reference_sha256: reference_sha256.into(),
        reference_component: "primary-whole-assembly".into(),
        reference_concept_ids: vec![],
    }
}
fn record(output: &Path) -> PairRecord {
    serde_json::from_slice(&std::fs::read(output.join("result.json")).unwrap()).unwrap()
}
#[test]
fn saved_pair_requires_expected_identities_component_and_concepts() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("result");
    let query = fixture("identical.q.fa");
    let reference = fixture("identical.r.fa");
    let result = cargo_bin_cmd!("terobacter-dev")
        .args([
            "anim-pair",
            "--query-id",
            "Q",
            "--reference-id",
            "R",
            "--reference-component",
            "primary-whole-assembly",
            "--reference-concept-id",
            "concept-a",
            "--reference-concept-id",
            "concept-b",
        ])
        .arg("--query")
        .arg(&query)
        .arg("--reference")
        .arg(&reference)
        .arg("--output")
        .arg(&out)
        .output()
        .unwrap();
    assert!(result.status.success());
    let qhash = hash_file(&query).unwrap();
    let rhash = hash_file(&reference).unwrap();
    let expected = ExpectedPair {
        reference_concept_ids: vec!["concept-a".into(), "concept-b".into()],
        ..expected_pair(&qhash, &rhash)
    };
    assert!(verify_pair(&out, &expected).is_ok());
    for mismatched in [
        ExpectedPair {
            query_id: "wrong-query".into(),
            ..expected.clone()
        },
        ExpectedPair {
            query_sha256: "0".repeat(64),
            ..expected.clone()
        },
        ExpectedPair {
            reference_id: "wrong-reference".into(),
            ..expected.clone()
        },
        ExpectedPair {
            reference_sha256: "0".repeat(64),
            ..expected.clone()
        },
        ExpectedPair {
            reference_component: "wrong-component".into(),
            ..expected.clone()
        },
        ExpectedPair {
            reference_concept_ids: vec!["fabricated-concept".into()],
            ..expected.clone()
        },
        ExpectedPair {
            reference_concept_ids: vec!["concept-a".into()],
            ..expected.clone()
        },
    ] {
        assert!(
            verify_pair(&out, &mismatched).is_err(),
            "mismatched expected context accepted: {mismatched:?}"
        );
    }
    let original = std::fs::read(out.join("result.json")).unwrap();
    for (role, field, value) in [
        (
            "reference",
            "component",
            serde_json::json!("wrong-component"),
        ),
        (
            "reference",
            "concept_ids",
            serde_json::json!(["fabricated-concept"]),
        ),
        ("reference", "id", serde_json::json!("wrong-reference")),
        ("query", "id", serde_json::json!("wrong-query")),
        ("query", "component", serde_json::json!("wrong-component")),
        (
            "query",
            "concept_ids",
            serde_json::json!(["fabricated-concept"]),
        ),
    ] {
        let mut altered: Value = serde_json::from_slice(&original).unwrap();
        altered[role][field] = value;
        std::fs::write(
            out.join("result.json"),
            serde_json::to_vec(&altered).unwrap(),
        )
        .unwrap();
        assert!(
            verify_pair(&out, &expected).is_err(),
            "altered {role} {field} accepted"
        );
    }
    std::fs::write(out.join("result.json"), &original).unwrap();
    let request = std::fs::read(out.join("request.json")).unwrap();
    for (field, value) in [
        ("query_id", serde_json::json!("wrong-query")),
        ("reference_id", serde_json::json!("wrong-reference")),
        ("reference_component", serde_json::json!("wrong-component")),
        (
            "reference_concept_ids",
            serde_json::json!(["fabricated-concept"]),
        ),
        ("expect_query_sha256", serde_json::json!("0".repeat(64))),
        ("expect_reference_sha256", serde_json::json!("0".repeat(64))),
    ] {
        let mut altered: Value = serde_json::from_slice(&request).unwrap();
        altered[field] = value;
        std::fs::write(
            out.join("request.json"),
            serde_json::to_vec(&altered).unwrap(),
        )
        .unwrap();
        assert!(
            verify_pair(&out, &expected).is_err(),
            "altered request {field} accepted"
        );
    }
    std::fs::write(out.join("request.json"), &request).unwrap();
    assert!(verify_pair(&out, &expected).is_ok());
}

#[test]
fn saved_pair_resolves_default_identities_from_input_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("result");
    let query = fixture("identical.q.fa");
    let reference = fixture("identical.r.fa");
    cargo_bin_cmd!("terobacter-dev")
        .arg("anim-pair")
        .arg("--query")
        .arg(&query)
        .arg("--reference")
        .arg(&reference)
        .arg("--output")
        .arg(&out)
        .assert()
        .success();
    let qhash = hash_file(&query).unwrap();
    let rhash = hash_file(&reference).unwrap();
    let expected = ExpectedPair {
        query_id: qhash.clone(),
        reference_id: rhash.clone(),
        ..expected_pair(&qhash, &rhash)
    };
    assert!(verify_pair(&out, &expected).is_ok());
}
#[test]
fn stale_input_hash_returns_incomplete_evidence_without_native_measurement() {
    let dir = tempfile::tempdir().unwrap();
    let output_dir = dir.path().join("result");
    let output = cargo_bin_cmd!("terobacter-dev")
        .arg("anim-pair")
        .arg("--query")
        .arg(fixture("identical.q.fa"))
        .arg("--reference")
        .arg(fixture("identical.r.fa"))
        .arg("--expect-query-sha256")
        .arg("0".repeat(64))
        .arg("--output")
        .arg(&output_dir)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report = record(&output_dir);
    assert_eq!(report.status, "incomplete");
    assert!(report.error.unwrap().contains("query SHA256 mismatch"));
    assert!(report.orientations.is_empty());
    assert!(output_dir.join("inputs/query.fa").is_file());
}
#[test]
fn reciprocal_native_pair_binds_roles_and_rejects_stale_receipts() {
    let dir = tempfile::tempdir().unwrap();
    let output_dir = dir.path().join("result with spaces");
    let query = fixture("cropped.q.fa");
    let reference = fixture("identical.r.fa");
    let output = pair(&query, &reference, &output_dir);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let qhash = hash_file(&query).unwrap();
    let rhash = hash_file(&reference).unwrap();
    let report = verify_pair(&output_dir, &expected_pair(&qhash, &rhash)).unwrap();
    for direction in ["QR", "RQ"] {
        let orientation = &report.orientations[direction];
        let metrics = orientation.metrics.as_ref().unwrap();
        assert_eq!(
            (metrics.reference_length, metrics.query_length),
            (120000, 96000)
        );
        assert_eq!(
            (metrics.reference_covered, metrics.query_covered),
            (96000, 96000)
        );
        assert_eq!((metrics.d, metrics.e), (192000, 0));
        assert!(metrics.coverage_at_least(true, 80, 1).unwrap());
        assert_eq!(
            orientation.receipt.as_ref().unwrap().native_exit_code,
            Some(0)
        );
    }
    assert_eq!(report.orientations["QR"].native_reference_id, "R");
    assert_eq!(report.orientations["RQ"].native_reference_id, "Q");
    assert!(verify_pair(&output_dir, &expected_pair(&"0".repeat(64), &rhash)).is_err());
    let original = std::fs::read(output_dir.join("result.json")).unwrap();
    let mut stale: Value = serde_json::from_slice(&original).unwrap();
    stale["method"]["build_sha256"] = Value::String("0".repeat(64));
    std::fs::write(
        output_dir.join("result.json"),
        serde_json::to_vec(&stale).unwrap(),
    )
    .unwrap();
    assert!(verify_pair(&output_dir, &expected_pair(&qhash, &rhash)).is_err());
    std::fs::write(output_dir.join("result.json"), &original).unwrap();
    std::fs::write(output_dir.join("QR/native.filter"), "corrupt").unwrap();
    assert!(verify_pair(&output_dir, &expected_pair(&qhash, &rhash)).is_err());
    // Output directories are never reused, even when an existing result is stale.
    assert!(!pair(&query, &reference, &output_dir).status.success());
    assert_eq!(
        std::fs::read(output_dir.join("result.json")).unwrap(),
        original
    );
}
#[test]
fn no_alignment_is_successful_with_undefined_identity_and_zero_coverage() {
    let dir = tempfile::tempdir().unwrap();
    let output_dir = dir.path().join("result");
    let output = pair(
        &fixture("no_alignment.q.fa"),
        &fixture("no_alignment.r.fa"),
        &output_dir,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = record(&output_dir);
    assert_eq!(report.status, "complete");
    for orientation in report.orientations.values() {
        assert_eq!(orientation.status, "no_alignment");
        let metrics = orientation.metrics.as_ref().unwrap();
        assert_eq!((metrics.d, metrics.e, metrics.blocks), (0, 0, 0));
        assert_eq!(metrics.identity().unwrap(), None);
        assert_eq!((metrics.reference_covered, metrics.query_covered), (0, 0));
    }
}
#[test]
fn upstream_missing_file_cannot_terminate_the_coordinator() {
    use std::io::Write;
    use std::process::Stdio;
    let dir = tempfile::tempdir().unwrap();
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin!("terobacter-dev"))
        .env("TERO_INTERNAL_ANIM_WORKER", "1")
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let request = serde_json::json!({"reference":"missing.fa","query":"missing.fa","prefix":"native",
        "memory_mib":6144,"parent_pid":std::process::id(),"wall_seconds":10.0});
    child
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(&request).unwrap().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let completion: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{error}; worker stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    assert_ne!(completion["exit_code"], 0);
    assert!(!completion["message"].as_str().unwrap().is_empty());
}

#[test]
fn concurrent_isolated_workers_preserve_literal_ambiguous_identity() {
    let run = || {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("result");
        assert!(
            pair(&fixture("ambiguous.q.fa"), &fixture("ambiguous.r.fa"), &out)
                .status
                .success()
        );
        record(&out)
    };
    let a = std::thread::spawn(run);
    let b = std::thread::spawn(run);
    let (a, b) = (a.join().unwrap(), b.join().unwrap());
    for direction in ["QR", "RQ"] {
        let metrics = a.orientations[direction].metrics.as_ref().unwrap();
        assert_eq!((metrics.d, metrics.e), (240000, 2400));
        assert_eq!(Some(metrics), b.orientations[direction].metrics.as_ref());
    }
}

#[test]
fn en4_development_parser_uses_the_production_weighted_metric_reader() {
    let dir = tempfile::tempdir().unwrap();
    let refs = dir.path().join("refs.json");
    let queries = dir.path().join("queries.json");
    std::fs::write(&refs, "{\"R\":40}").unwrap();
    std::fs::write(&queries, "{\"Q\":40}").unwrap();
    let output = cargo_bin_cmd!("terobacter-dev")
        .arg("anim-parse")
        .arg("--filtered-delta")
        .arg(fixture("weighted.filter"))
        .arg("--reference-lengths")
        .arg(refs)
        .arg("--query-lengths")
        .arg(queries)
        .output()
        .unwrap();
    assert!(output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        (result["D"].as_u64(), result["E"].as_u64()),
        (Some(33), Some(5))
    );
}

#[test]
fn en4_malformed_record_headers_fail_without_metrics_or_panic() {
    let dir = tempfile::tempdir().unwrap();
    let refs = dir.path().join("refs.json");
    let queries = dir.path().join("queries.json");
    let delta = dir.path().join("malformed.filter");
    std::fs::write(&refs, "{\"r\":100}").unwrap();
    std::fs::write(&queries, "{\"q\":100}").unwrap();
    for header in ["unknown q nope 100", "r unknown 100 nope"] {
        for block in ["", "1 10 1 10 0 0 0\n0\n"] {
            std::fs::write(&delta, format!("r.fa q.fa\nNUCMER\n>{header}\n{block}")).unwrap();
            let output = cargo_bin_cmd!("terobacter-dev")
                .arg("anim-parse")
                .arg("--filtered-delta")
                .arg(&delta)
                .arg("--reference-lengths")
                .arg(&refs)
                .arg("--query-lengths")
                .arg(&queries)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert!(stderr.contains("invalid ANIm evidence"));
            assert!(!stderr.contains("panicked"));
        }
    }
}

#[test]
fn sigterm_cancellation_retains_incomplete_pair_and_stops_native_worker() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("cancelled");
    let mut child = Command::new(assert_cmd::cargo::cargo_bin!("terobacter-dev"))
        .args(["anim-pair", "--query"])
        .arg(fixture("identical.q.fa"))
        .arg("--reference")
        .arg(fixture("identical.r.fa"))
        .arg("--output")
        .arg(&out)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while !out.join("QR/request.json").exists() {
        assert!(start.elapsed() < Duration::from_secs(10));
        assert!(
            child.try_wait().unwrap().is_none(),
            "pair completed before cancellation"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    unsafe {
        libc::kill(child.id() as i32, libc::SIGTERM);
    }
    assert!(!child.wait().unwrap().success());
    let report = record(&out);
    assert_eq!(report.status, "incomplete");
    assert!(report.error.as_ref().unwrap().contains("cancelled"));
    for orientation in report.orientations.values() {
        if let Some(receipt) = &orientation.receipt {
            // waitpid has reaped every started worker before the result is published.
            assert_eq!(unsafe { libc::kill(receipt.worker_pid as i32, 0) }, -1);
        }
    }
}

#[test]
fn crlf_and_surrounding_whitespace_preserve_identical_whole_sequences() {
    let dir = tempfile::tempdir().unwrap();
    let source = std::fs::read_to_string(fixture("identical.q.fa")).unwrap();
    let sequence: String = source
        .lines()
        .filter(|line| !line.starts_with('>'))
        .collect();
    let query = dir.path().join("crlf.fa");
    let reference = dir.path().join("whitespace.fa");
    let mut q = String::from(">Q description\r\n");
    let mut r = String::from(">R description\r\n");
    for chunk in sequence.as_bytes().chunks(10) {
        let bases = std::str::from_utf8(chunk).unwrap();
        q.push_str(&format!("{bases}\r\n"));
        r.push_str(&format!(" \t{bases} \t\r\n"));
    }
    std::fs::write(&query, q).unwrap();
    std::fs::write(&reference, r).unwrap();
    let out = dir.path().join("result");
    let result = pair(&query, &reference, &out);
    assert!(result.status.success());
    let report = verify_pair(
        &out,
        &expected_pair(&hash_file(&query).unwrap(), &hash_file(&reference).unwrap()),
    )
    .unwrap();
    for orientation in report.orientations.values() {
        let m = orientation.metrics.as_ref().unwrap();
        assert_eq!(
            (m.d, m.e, m.reference_covered, m.query_covered),
            (240000, 0, 120000, 120000)
        );
    }
    assert_eq!(
        std::fs::read(out.join("inputs/query.fa")).unwrap(),
        std::fs::read(query).unwrap()
    );
}
