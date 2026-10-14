//! Reuse of reciprocal ANIm pairs: only for the same comparison, verified again, never
//! replaced, and never holding a failure. Synthetic native package only.
use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
use std::path::{Path, PathBuf};
use terobacter::cli::cache::{copy_tree, AnimCache, CacheKey, Lookup};
mod common;
use common::assembly::NativeCase;

struct Prepared {
    dir: tempfile::TempDir,
    pin: String,
    anim_pin: String,
    query: PathBuf,
    seq: Vec<u8>,
}

impl Prepared {
    fn new(case: NativeCase) -> Self {
        let (dir, pin, seq) = common::assembly::native_case(case);
        let query = dir.path().join("query.fa");
        std::fs::write(&query, fasta(&seq, b"")).unwrap();
        let anim_pin = common::package::sha256_hex(
            &std::fs::read(dir.path().join("fastas/anim.json")).unwrap(),
        );
        Self {
            dir,
            pin,
            anim_pin,
            query,
            seq,
        }
    }

    fn cache(&self) -> PathBuf {
        self.dir.path().join("cache")
    }

    /// A v2 run of `queries` into the new evidence directory `evidence`, sharing one cache.
    fn run(&self, evidence: &str, extra: &[&str], queries: &[&Path]) -> (Option<i32>, Value) {
        let output = cargo_bin_cmd!("tero")
            .arg("--reference-package")
            .arg(self.dir.path().join("package"))
            .args(["--expected-sha256", &self.pin])
            .args(["--expected-anim-sha256", &self.anim_pin])
            .arg("--anim-reference-package")
            .arg(self.dir.path().join("fastas"))
            .arg("--anim-output")
            .arg(self.dir.path().join(evidence))
            .arg("--anim-cache")
            .arg(self.cache())
            .args(extra)
            .args(queries)
            .output()
            .unwrap();
        let report = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
            panic!("{}", String::from_utf8_lossy(&output.stderr));
        });
        (output.status.code(), report)
    }

    fn entries(&self) -> Vec<PathBuf> {
        let mut entries: Vec<_> = std::fs::read_dir(self.cache().join("anim-pairs-1"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        entries
    }
}

fn fasta(seq: &[u8], extra: &[u8]) -> Vec<u8> {
    [b">synthetic\n".as_slice(), seq, b"\n", extra].concat()
}

fn statuses(report: &Value) -> Vec<String> {
    report["anim_execution"]["pairs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["cache"]["status"].as_str().unwrap().to_owned())
        .collect()
}

fn assert_rescued(report: &Value) {
    assert_eq!(report["rescue_status"], "rescued", "{report}");
    assert_eq!(report["species_id"], "SYN-SP01");
    assert_eq!(report["counts"]["completed"], 5);
    for pair in report["anim_execution"]["pairs"].as_array().unwrap() {
        assert_eq!(pair["status"], "complete");
        assert_eq!(
            pair["record"]["query"]["sha256"],
            report["input"]["query_sha256"]
        );
        let directory = Path::new(pair["directory"].as_str().unwrap());
        assert!(directory.join("result.json").exists());
    }
}

#[test]
fn verified_pairs_are_reused_only_for_the_same_query_bytes_and_settings() {
    let prepared = Prepared::new(NativeCase::Rescue);
    let (code, first) = prepared.run("first", &[], &[&prepared.query]);
    assert_eq!(code, Some(0), "{first}");
    assert_rescued(&first);
    assert_eq!(statuses(&first), ["stored"; 5]);
    assert_eq!(prepared.entries().len(), 5);

    // The same bytes under another name, twice in one batch, reuse every pair.
    let copy = prepared.dir.path().join("copy.fa");
    std::fs::copy(&prepared.query, &copy).unwrap();
    let (code, again) = prepared.run("again", &[], &[&copy, &prepared.query]);
    assert_eq!(code, Some(0), "{again}");
    for report in again.as_array().unwrap() {
        assert_rescued(report);
        assert_eq!(statuses(report), ["reused"; 5]);
        for key in [
            "species_id",
            "reasons",
            "counts",
            "call_basis",
            "competitor_evaluation",
        ] {
            assert_eq!(report[key], first[key], "{key}");
        }
        for (reused, measured) in report["anim_execution"]["pairs"]
            .as_array()
            .unwrap()
            .iter()
            .zip(first["anim_execution"]["pairs"].as_array().unwrap())
        {
            assert_eq!(reused["record"], measured["record"]);
            assert_eq!(
                reused["cache"]["key_sha256"],
                measured["cache"]["key_sha256"]
            );
        }
    }

    // Different query bytes are a different comparison.
    let extended = prepared.dir.path().join("extended.fa");
    std::fs::write(&extended, fasta(&prepared.seq, b">short\nACGTN\n")).unwrap();
    let (code, report) = prepared.run("extended", &[], &[&extended]);
    assert_eq!(code, Some(0), "{report}");
    assert_rescued(&report);
    assert_eq!(statuses(&report), ["stored"; 5]);

    // So are different limits, which decide whether a pair can complete.
    for (name, setting) in [
        ("memory", ["--anim-memory-mib", "2048"]),
        ("timeout", ["--anim-pair-seconds", "300"]),
    ] {
        let (code, report) = prepared.run(name, &setting, &[&prepared.query]);
        assert_eq!(code, Some(0), "{report}");
        assert_rescued(&report);
        assert_eq!(statuses(&report), ["stored"; 5], "{name}");
    }
    assert_eq!(prepared.entries().len(), 20);
}

type Mutation<'a> = Box<dyn Fn(&mut CacheKey) + 'a>;

#[test]
fn every_key_field_must_match_even_for_an_entry_planted_at_its_address() {
    let prepared = Prepared::new(NativeCase::Rescue);
    let (code, report) = prepared.run("first", &[], &[&prepared.query]);
    assert_eq!(code, Some(0), "{report}");
    let cache = AnimCache::open(&prepared.cache()).unwrap();
    let entry = &prepared.entries()[0];
    let key: CacheKey =
        serde_json::from_slice(&std::fs::read(entry.join("key.json")).unwrap()).unwrap();
    assert!(matches!(cache.lookup(&key), Lookup::Hit(_)));
    let other = "0".repeat(64);
    let mutations: Vec<(&str, Mutation)> = vec![
        ("schema", Box::new(|k| k.schema.push('x'))),
        ("query bytes", Box::new(|k| k.query.sha256 = other.clone())),
        ("query component", Box::new(|k| k.query.component.push('x'))),
        (
            "query scope",
            Box::new(|k| k.query.scope = "chromosome".into()),
        ),
        (
            "reference ID",
            Box::new(|k| k.reference.reference_id.push('x')),
        ),
        (
            "reference bytes",
            Box::new(|k| k.reference.sha256 = other.clone()),
        ),
        (
            "reference component",
            Box::new(|k| k.reference.component.push('x')),
        ),
        (
            "reference concepts",
            Box::new(|k| k.reference.concept_ids.push("SYN-OTHER".into())),
        ),
        (
            "reference scope",
            Box::new(|k| k.reference.scope = "chromosome".into()),
        ),
        (
            "bound package",
            Box::new(|k| k.reference.package_sha256 = other.clone()),
        ),
        ("method profile", Box::new(|k| k.method.profile.push('x'))),
        (
            "native build",
            Box::new(|k| k.method.build_sha256 = other.clone()),
        ),
        (
            "implementation version",
            Box::new(|k| k.method.implementation_version.push('x')),
        ),
        (
            "pair timeout",
            Box::new(|k| k.settings.pair_timeout_nanoseconds += 1),
        ),
        ("memory", Box::new(|k| k.settings.memory_mib += 1)),
    ];
    for (label, mutate) in mutations {
        let mut changed = key.clone();
        mutate(&mut changed);
        assert_ne!(changed, key, "{label} did not change the key");
        assert!(matches!(cache.lookup(&changed), Lookup::Miss), "{label}");
        // A valid entry for the original comparison placed at the changed key's address.
        let planted = cache.entry(&changed);
        copy_tree(entry, &planted).unwrap();
        match cache.lookup(&changed) {
            Lookup::Rejected(why) => assert!(why.contains("key differs"), "{label}: {why}"),
            other => panic!("{label}: planted entry was {other:?}"),
        }
        // A planted entry whose saved key also claims the changed comparison is still
        // checked against the saved record wherever the record carries that field.
        std::fs::write(
            planted.join("key.json"),
            serde_json::to_vec(&changed).unwrap(),
        )
        .unwrap();
        let bound_by_record = [
            "query bytes",
            "reference ID",
            "reference bytes",
            "reference component",
            "reference concepts",
            "method profile",
            "native build",
            "implementation version",
            "memory",
        ];
        if bound_by_record.contains(&label) {
            assert!(
                matches!(cache.lookup(&changed), Lookup::Rejected(_)),
                "{label}"
            );
        }
        std::fs::remove_dir_all(&planted).unwrap();
    }
}

#[test]
fn a_tampered_entry_is_rejected_measured_again_and_left_in_place() {
    let prepared = Prepared::new(NativeCase::Rescue);
    let (code, first) = prepared.run("first", &[], &[&prepared.query]);
    assert_eq!(code, Some(0), "{first}");
    let target = first["anim_execution"]["pairs"][0]["cache"]["key_sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    let filter = prepared
        .cache()
        .join("anim-pairs-1")
        .join(&target)
        .join("pair/QR/native.filter");
    let mut bytes = std::fs::read(&filter).unwrap();
    bytes.extend_from_slice(b"tampered\n");
    std::fs::write(&filter, &bytes).unwrap();
    let (code, report) = prepared.run("again", &[], &[&prepared.query]);
    assert_eq!(code, Some(0), "{report}");
    assert_rescued(&report);
    let pairs = report["anim_execution"]["pairs"].as_array().unwrap();
    assert_eq!(pairs[0]["cache"]["status"], "rejected");
    assert!(pairs[0]["cache"]["rejection"]
        .as_str()
        .unwrap()
        .contains("stale native artifact"));
    assert_ne!(
        pairs[0]["record"],
        first["anim_execution"]["pairs"][0]["record"]
    );
    assert_eq!(statuses(&report)[1..], ["reused"; 4]);
    assert_eq!(std::fs::read(&filter).unwrap(), bytes);
}

#[test]
fn saved_native_time_counts_against_the_query_budget() {
    let prepared = Prepared::new(NativeCase::Rescue);
    let limits = [
        "--anim-pair-seconds",
        "1800",
        "--anim-query-seconds",
        "7200",
    ];
    let (code, first) = prepared.run("first", &limits, &[&prepared.query]);
    assert_eq!(code, Some(0), "{first}");
    // Record 1500 seconds of native time in every entry. Receipt times are provenance, not
    // part of the verified artifacts, so the entries stay valid.
    for entry in prepared.entries() {
        let path = entry.join("pair/result.json");
        let mut record: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        record["orientations"]["QR"]["receipt"]["wall_seconds"] = 1500.0.into();
        record["orientations"]["RQ"]["receipt"]["wall_seconds"] = 0.0.into();
        std::fs::write(&path, serde_json::to_vec_pretty(&record).unwrap()).unwrap();
    }
    // Four reuses charge 6000 of the 7200 seconds, so the fifth no longer fits and is
    // measured again instead.
    let (code, report) = prepared.run("again", &limits, &[&prepared.query]);
    assert_eq!(code, Some(0), "{report}");
    assert_rescued(&report);
    assert_eq!(statuses(&report)[..4], ["reused"; 4]);
    let last = &report["anim_execution"]["pairs"][4]["cache"];
    assert_eq!(last["status"], "rejected");
    assert!(last["rejection"].as_str().unwrap().contains("time"));
}

#[test]
fn failed_pairs_never_enter_the_cache() {
    let prepared = Prepared::new(NativeCase::MalformedReference);
    let (code, report) = prepared.run("first", &[], &[&prepared.query]);
    assert_eq!(code, Some(1), "{report}");
    assert_eq!(report["species_id"], Value::Null);
    let pairs = report["anim_execution"]["pairs"].as_array().unwrap();
    let failed: Vec<_> = pairs.iter().filter(|p| p["status"] == "failed").collect();
    assert!(!failed.is_empty(), "{report}");
    for pair in &failed {
        assert_eq!(pair["cache"]["status"], "not_stored");
        let key = pair["cache"]["key_sha256"].as_str().unwrap();
        assert!(!prepared.cache().join("anim-pairs-1").join(key).exists());
    }
    let stored = statuses(&report).iter().filter(|s| *s == "stored").count();
    assert_eq!(prepared.entries().len(), stored);
}

#[test]
fn concurrent_v2_queries_keep_their_own_evidence_and_share_the_cache_safely() {
    let prepared = Prepared::new(NativeCase::Rescue);
    let copy = prepared.dir.path().join("copy.fa");
    std::fs::copy(&prepared.query, &copy).unwrap();
    let extended = prepared.dir.path().join("extended.fa");
    std::fs::write(&extended, fasta(&prepared.seq, b">short\nACGTN\n")).unwrap();
    let bad = prepared.dir.path().join("bad.fa");
    std::fs::write(&bad, "not FASTA").unwrap();
    let (code, report) = prepared.run(
        "evidence",
        &["--jobs", "3"],
        &[&prepared.query, &bad, &copy, &extended],
    );
    assert_eq!(code, Some(3), "{report}");
    let reports = report.as_array().unwrap();
    assert_eq!(reports[1]["execution"]["stage"], "input");
    for (ordinal, report) in [(1, &reports[0]), (3, &reports[2]), (4, &reports[3])] {
        assert_rescued(report);
        assert_eq!(report["input"]["ordinal"], ordinal);
        for pair in report["anim_execution"]["pairs"].as_array().unwrap() {
            assert!(pair["directory"]
                .as_str()
                .unwrap()
                .contains(&format!("query-{ordinal:06}")));
            let status = pair["cache"]["status"].as_str().unwrap();
            assert!(
                ["stored", "already_stored", "reused"].contains(&status),
                "{status}"
            );
        }
    }
    // Two distinct query byte strings, five references each.
    assert_eq!(prepared.entries().len(), 10);
    let leftovers = prepared
        .entries()
        .iter()
        .filter(|e| e.file_name().unwrap().to_str().unwrap().starts_with('.'))
        .count();
    assert_eq!(leftovers, 0);
}

#[test]
fn v1_refuses_the_anim_cache_option() {
    let prepared = Prepared::new(NativeCase::Rescue);
    let output = cargo_bin_cmd!("tero")
        .arg("--no-anim")
        .arg("--reference-package")
        .arg(prepared.dir.path().join("package"))
        .args(["--expected-sha256", &prepared.pin])
        .arg("--anim-cache")
        .arg(prepared.cache())
        .arg(&prepared.query)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(5));
    assert!(!prepared.cache().exists());
}
