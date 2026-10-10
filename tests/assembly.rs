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
fn requested_native_v2_is_an_explicit_unavailable_policy_failure() {
    let output = cargo_bin_cmd!("tero")
        .args([
            "--reference-package",
            FIXTURE_DIR,
            "--expected-sha256",
            PINNED_MANIFEST_SHA256,
            "--policy",
            "conservative-assembly-anim-v2",
            "missing.fa",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(5));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["execution"]["stage"], "policy");
    assert_eq!(report["policy"]["id"], "conservative-assembly-anim-v2");
    assert_eq!(report["assignment"]["species_id"], Value::Null);
}

fn native_package() -> (tempfile::TempDir, String, Vec<u8>) {
    use common::package::{sha256_hex, Synthetic, EXPORT_PATH, SOURCE_PATH};
    use serde_json::json;
    use skani::{params::SketchParams, sketch_db::SketchDbWriter, types::Sketch};
    let mut package = Synthetic::new();
    let verified = package.verify().unwrap();
    let component = verified.component("primary_whole").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("database");
    std::fs::create_dir(&database).unwrap();
    let params = SketchParams::new(1000, 125, 15, false, false);
    let original = common::dna(200_003, 42);
    let mut markers = Vec::new();
    let mut writer = SketchDbWriter::new(database.to_str().unwrap()).unwrap();
    for (i, reference) in component.references.iter().enumerate() {
        let mut seq = original.clone();
        if i > 0 {
            let mut state = 927_u64;
            for base in &mut seq {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                if state % 100 < 8 {
                    *base = match *base {
                        b'A' => b'C',
                        b'C' => b'G',
                        b'G' => b'T',
                        _ => b'A',
                    };
                }
            }
        }
        let mut sketch = Sketch::new(1000, 125, 15, reference.search_label.clone(), false);
        sketch.contigs.push("synthetic".into());
        sketch.contig_lengths.push(seq.len() as u32);
        sketch.total_sequence_length = seq.len();
        skani::seeding::fmh_seeds(&seq, &params, 0, &mut sketch, true);
        writer.add_sketch(&params, &sketch).unwrap();
        markers.push(Sketch::get_markers_only(&sketch));
    }
    writer.finalize(database.to_str().unwrap()).unwrap();
    std::fs::write(
        database.join("markers.bin"),
        bincode::serialize(&(params, markers)).unwrap(),
    )
    .unwrap();
    let mut replacements = std::collections::BTreeMap::new();
    for file in ["index.db", "sketches.db", "markers.bin"] {
        let asset = &component.databases[file];
        let bytes = std::fs::read(database.join(file)).unwrap();
        replacements.insert(
            asset.source_path.clone(),
            json!({"path": asset.source_path,
            "bytes": bytes.len(), "sha256": sha256_hex(&bytes)}),
        );
        package.rewrite(&asset.path, bytes);
    }
    // Index records use bincode's pinned u64 lengths and little-endian offsets.
    let bytes = std::fs::read(database.join("index.db")).unwrap();
    let mut cursor = std::io::Cursor::new(bytes);
    use std::io::Read;
    fn u64_at(cursor: &mut std::io::Cursor<Vec<u8>>) -> u64 {
        let mut bytes = [0; 8];
        cursor.read_exact(&mut bytes).unwrap();
        u64::from_le_bytes(bytes)
    }
    let count = u64_at(&mut cursor);
    let mut entries = Vec::new();
    for _ in 0..count {
        let size = u64_at(&mut cursor);
        let mut label = vec![0; size as usize];
        cursor.read_exact(&mut label).unwrap();
        let offset = u64_at(&mut cursor);
        let size = u64_at(&mut cursor);
        entries.push(
            json!({"path": String::from_utf8(label).unwrap(), "offset": offset, "bytes": size}),
        );
    }
    entries.sort_by_key(|entry| entry["path"].as_str().unwrap().to_owned());
    fn replace(value: &mut Value, replacements: &std::collections::BTreeMap<String, Value>) {
        if let Some(replacement) = value["path"].as_str().and_then(|p| replacements.get(p)) {
            if value.get("sha256").is_some() {
                *value = replacement.clone();
                return;
            }
        }
        match value {
            Value::Object(map) => {
                for child in map.values_mut() {
                    replace(child, replacements);
                }
            }
            Value::Array(array) => {
                for child in array {
                    replace(child, replacements);
                }
            }
            _ => {}
        }
    }
    package.edit_json(SOURCE_PATH, |source| {
        replace(source, &replacements);
        let primary = source["components"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|c| c["component_id"] == "primary_whole")
            .unwrap();
        primary["database_index_entries"] = json!(entries);
    });
    let source_hash = sha256_hex(package.file(SOURCE_PATH));
    let database_bytes: usize = package
        .files()
        .iter()
        .filter(|(p, _)| p.starts_with("databases/"))
        .map(|(_, b)| b.len())
        .sum();
    package.edit_json(EXPORT_PATH, |export| {
        export["source_manifest_sha256"] = source_hash.clone().into();
        export["local_validation"]["database_bytes"] = database_bytes.into();
    });
    package.edit_json(common::package::LICENCE_PATH, |licence| {
        licence["source_manifest_sha256"] = source_hash.clone().into();
    });
    package.verify().unwrap();
    let target = dir.path().join("package");
    package.write_to(&target);
    (dir, package.manifest_sha256(), original)
}

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
    let output = cargo_bin_cmd!("tero")
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
    let output = cargo_bin_cmd!("tero")
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
