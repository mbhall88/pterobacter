//! Assembly lists, sample sheets and sample names. A name is reporting metadata: every
//! measurement must equal the one the same file gives as a positional argument.
use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
use std::path::{Path, PathBuf};
mod common;

struct Run {
    report: Value,
    code: Option<i32>,
    stderr: String,
}

fn ptero(package: &Path, pin: &str, args: &[&std::ffi::OsStr]) -> Run {
    let output = cargo_bin_cmd!("ptero")
        .arg("--reference-package")
        .arg(package)
        .args(["--expected-sha256", pin])
        .args(args)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let report = if output.stdout.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("{e}: {stderr}"))
    };
    Run {
        report,
        code: output.status.code(),
        stderr,
    }
}

fn os(text: &str) -> &std::ffi::OsStr {
    text.as_ref()
}

/// Paths where two reports differ.
fn differences(left: &Value, right: &Value, at: &str, found: &mut Vec<String>) {
    match (left, right) {
        (Value::Object(l), Value::Object(r)) => {
            for key in l
                .keys()
                .chain(r.keys())
                .collect::<std::collections::BTreeSet<_>>()
            {
                differences(
                    l.get(key).unwrap_or(&Value::Null),
                    r.get(key).unwrap_or(&Value::Null),
                    &format!("{at}.{key}"),
                    found,
                );
            }
        }
        (Value::Array(l), Value::Array(r)) if l.len() == r.len() => {
            for (index, (l, r)) in l.iter().zip(r).enumerate() {
                differences(l, r, &format!("{at}[{index}]"), found);
            }
        }
        _ if left != right => found.push(at.into()),
        _ => {}
    }
}

/// Every field of two reports agrees except the elapsed time, which is a measurement of the
/// run and not of the assembly.
fn assert_same(left: &Value, right: &Value) {
    let mut found = Vec::new();
    differences(left, right, "", &mut found);
    found.retain(|path| !path.ends_with(".anim_execution.wall_seconds"));
    assert!(found.is_empty(), "reports differ at {found:?}");
}

/// The report without the two fields that record where an input came from.
fn without_source(report: &Value) -> Value {
    let mut report = report.clone();
    for item in match &mut report {
        Value::Array(items) => items.iter_mut().collect::<Vec<_>>(),
        other => vec![other],
    } {
        let input = item["input"].as_object_mut().unwrap();
        assert!(input.remove("sample_name").is_some());
        assert!(input.remove("source").is_some());
    }
    report
}

fn gzip(path: &Path) -> Vec<u8> {
    let output = std::process::Command::new("gzip")
        .args(["-n", "-c"])
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success());
    output.stdout
}

/// Two queries, one compressed, in a directory whose name has a space and a non-ASCII letter.
fn queries(root: &Path, seq: &[u8]) -> (PathBuf, PathBuf, PathBuf) {
    let dir = root.join("runs ü");
    std::fs::create_dir(&dir).unwrap();
    let plain = dir.join("q 1.fa");
    std::fs::write(
        &plain,
        [b">synthetic\n".as_slice(), seq, b"\n>short\nACGTN\n"].concat(),
    )
    .unwrap();
    let compressed = dir.join("q2 é.fa.gz");
    std::fs::write(&compressed, gzip(&plain)).unwrap();
    (dir, plain, compressed)
}

#[test]
fn lists_sheets_and_names_reproduce_positional_reports_exactly() {
    let (dir, pin, seq) = common::assembly::native_package();
    let package = dir.path().join("package");
    let (runs, plain, compressed) = queries(dir.path(), &seq);
    let list = runs.join("list.txt");
    std::fs::write(&list, "q 1.fa\r\n\r\nq2 é.fa.gz\r\n").unwrap();
    let sheet = runs.join("samples.tsv");
    std::fs::write(
        &sheet,
        "sample\tpath\r\nIsolate A\tq 1.fa\r\nB-é\tq2 é.fa.gz\r\n",
    )
    .unwrap();
    for mode in [None, Some("--no-anim")] {
        let with = |extra: &[&std::ffi::OsStr]| {
            let mut args: Vec<&std::ffi::OsStr> = mode.map(os).into_iter().collect();
            args.extend_from_slice(extra);
            let run = ptero(&package, &pin, &args);
            assert_eq!(run.code, Some(0), "{mode:?} {}", run.stderr);
            run.report
        };
        let positional = with(&[plain.as_os_str(), compressed.as_os_str()]);
        let listed = with(&[os("--input-list"), list.as_os_str()]);
        let sheeted = with(&[os("-F"), sheet.as_os_str()]);
        let named = with(&[os("-N"), os("Isolate A"), plain.as_os_str()]);
        let single = with(&[plain.as_os_str()]);
        assert_same(&without_source(&listed), &without_source(&positional));
        assert_same(&without_source(&sheeted), &without_source(&positional));
        assert_same(&without_source(&named), &without_source(&single));
        assert_same(&named, &{
            let mut expected = single.clone();
            expected["input"]["sample_name"] = "Isolate A".into();
            expected
        });

        let reports = positional.as_array().unwrap();
        assert_eq!(reports[0]["input"]["file"], plain.to_str().unwrap());
        assert_eq!(reports[1]["input"]["file"], compressed.to_str().unwrap());
        assert_ne!(
            reports[0]["input"]["query_sha256"],
            reports[1]["input"]["query_sha256"]
        );
        assert_eq!(reports[0]["input"]["sample_name"], Value::Null);
        assert_eq!(reports[0]["input"]["source"]["kind"], "positional");

        let sheeted = sheeted.as_array().unwrap();
        for (index, (name, entry, line)) in [("Isolate A", "q 1.fa", 2), ("B-é", "q2 é.fa.gz", 3)]
            .into_iter()
            .enumerate()
        {
            let input = &sheeted[index]["input"];
            assert_eq!(input["sample_name"], name);
            assert_eq!(input["ordinal"], index + 1);
            assert_eq!(input["source"]["kind"], "sample_sheet");
            assert_eq!(input["source"]["file"], sheet.to_str().unwrap());
            assert_eq!(input["source"]["entry"], entry);
            assert_eq!(input["source"]["line"], line);
            // The resolved path, not the name, is the file and the skani query label.
            assert_eq!(input["file"], reports[index]["input"]["file"]);
        }
        assert_eq!(listed[1]["input"]["source"]["line"], 3);
        assert_eq!(listed[1]["input"]["sample_name"], Value::Null);
    }
}

#[test]
fn mixed_batch_keeps_order_names_and_status_in_every_output() {
    let (dir, pin, seq) = common::assembly::native_package();
    let package = dir.path().join("package");
    let (runs, _, _) = queries(dir.path(), &seq);
    std::fs::write(runs.join("reads.fq"), b"@r\nACGT\n+\n!!!!\n").unwrap();
    let sheet = runs.join("samples.tsv");
    std::fs::write(
        &sheet,
        "sample\tpath\nfirst ok\tq2 é.fa.gz\nmissing\tabsent.fa\nreads\treads.fq\nlast ok\tq 1.fa\n",
    )
    .unwrap();
    let reports = dir.path().join("reports");
    let run = ptero(
        &package,
        &pin,
        &[
            os("--no-anim"),
            os("-F"),
            sheet.as_os_str(),
            os("-d"),
            reports.as_os_str(),
        ],
    );
    assert_eq!(run.code, Some(3), "{}", run.stderr);
    let items = run.report.as_array().unwrap();
    let expected = [
        ("first ok", "complete", "SYN-SP01"),
        ("missing", "incomplete", ""),
        ("reads", "incomplete", ""),
        ("last ok", "complete", "SYN-SP01"),
    ];
    for (item, (name, status, species)) in items.iter().zip(expected) {
        assert_eq!(item["input"]["sample_name"], name);
        assert_eq!(item["execution"]["status"], status, "{name}");
        assert_eq!(
            item["assignment"]["species_id"].as_str().unwrap_or(""),
            species
        );
    }
    assert_eq!(items[1]["execution"]["stage"], "input");
    assert_eq!(
        items[1]["input"]["file"],
        runs.join("absent.fa").to_str().unwrap()
    );
    assert!(items[2]["input"]["query_sha256"].is_string());

    let saved: Value =
        serde_json::from_slice(&std::fs::read(reports.join("report.json")).unwrap()).unwrap();
    assert_eq!(saved, run.report);
    let calls = std::fs::read_to_string(reports.join("calls.tsv")).unwrap();
    let rows: Vec<Vec<&str>> = calls.lines().map(|l| l.split('\t').collect()).collect();
    assert_eq!(&rows[0][..3], ["assembly", "sample_name", "input_file"]);
    for (row, (index, (name, status, species))) in rows[1..].iter().zip(expected.iter().enumerate())
    {
        let cell = |column: &str| row[rows[0].iter().position(|c| *c == column).unwrap()];
        assert_eq!(cell("assembly"), (index + 1).to_string());
        assert_eq!(cell("sample_name"), *name);
        assert_eq!(cell("input_file"), items[index]["input"]["file"]);
        assert_eq!(cell("execution_status"), *status);
        assert_eq!(cell("species_id"), *species);
    }
    let evidence = std::fs::read_to_string(reports.join("evidence.tsv")).unwrap();
    let mut evidence = evidence.lines().map(|l| l.split('\t').collect::<Vec<_>>());
    assert_eq!(&evidence.next().unwrap()[..2], ["assembly", "sample_name"]);
    let named: std::collections::BTreeSet<(String, String)> = evidence
        .map(|row| (row[0].to_owned(), row[1].to_owned()))
        .collect();
    assert_eq!(
        named,
        [("1", "first ok"), ("4", "last ok")]
            .map(|(a, n)| (a.to_owned(), n.to_owned()))
            .into()
    );
    let markdown = std::fs::read_to_string(reports.join("report.md")).unwrap();
    for (index, (name, _, _)) in expected.iter().enumerate() {
        let heading = format!(
            "## Assembly {}: {name}\n\n**Input file:** `{}`\n",
            index + 1,
            items[index]["input"]["file"].as_str().unwrap()
        );
        assert!(markdown.contains(&heading), "{heading}");
    }
}

#[test]
fn markdown_shows_a_punctuated_name_literally_and_every_other_output_keeps_it_exactly() {
    let (dir, pin, seq) = common::assembly::native_package();
    let (_, plain, _) = queries(dir.path(), &seq);
    let name = r"<isolate> *A* _b_ `tick` [x](y) back\slash & #";
    let reports = dir.path().join("reports");
    let run = ptero(
        &dir.path().join("package"),
        &pin,
        &[
            os("--no-anim"),
            os("-N"),
            os(name),
            os("-d"),
            reports.as_os_str(),
            plain.as_os_str(),
        ],
    );
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    assert_eq!(run.report["input"]["sample_name"], name);
    assert_eq!(run.report["input"]["file"], plain.to_str().unwrap());
    let calls = std::fs::read_to_string(reports.join("calls.tsv")).unwrap();
    let row: Vec<&str> = calls.lines().nth(1).unwrap().split('\t').collect();
    assert_eq!(row[1], name);
    let markdown = std::fs::read_to_string(reports.join("report.md")).unwrap();
    let heading = format!(
        "## Assembly 1: {}\n\n**Input file:** `{}`\n",
        r"\<isolate\> \*A\* \_b\_ \`tick\` \[x\]\(y\) back\\slash \& \#",
        plain.to_str().unwrap()
    );
    assert!(markdown.contains(&heading), "{markdown}");
    assert!(!markdown.contains("<isolate>"), "{markdown}");
}

#[test]
fn invalid_mappings_and_conflicting_sources_are_refused_before_any_output() {
    let (dir, pin, seq) = common::assembly::native_package();
    let package = dir.path().join("package");
    let (runs, plain, _) = queries(dir.path(), &seq);
    let file = |name: &str, text: &[u8]| {
        let path = runs.join(name);
        std::fs::write(&path, text).unwrap();
        path
    };
    let empty = file("empty.txt", b"\r\n \n");
    let latin1 = file("latin1.txt", b"q 1.fa\nq\xe9.fa\n");
    let aliased = file("aliased.txt", "q 1.fa\n./q 1.fa\n".as_bytes());
    let names = file(
        "names.tsv",
        b"sample\tpath\nA\tq 1.fa\nA\tq2 \xc3\xa9.fa.gz\n",
    );
    let three = file("three.tsv", b"sample\tpath\nA\tq 1.fa\textra\n");
    let header = file("header.tsv", b"name\tfile\nA\tq 1.fa\n");
    let sheet = file("ok.tsv", b"sample\tpath\nA\tq 1.fa\n");
    let list = file("ok.txt", b"q 1.fa\n");
    let absent = runs.join("absent.txt");
    let cases: Vec<(Vec<&std::ffi::OsStr>, &str)> = vec![
        (vec![os("-f"), empty.as_os_str()], "names no assembly"),
        (
            vec![os("-f"), latin1.as_os_str()],
            "line 2: the file is not valid UTF-8",
        ),
        (
            vec![os("-f"), aliased.as_os_str()],
            "same assembly as line 1",
        ),
        (vec![os("-F"), names.as_os_str()], "duplicate sample name"),
        (
            vec![os("-F"), three.as_os_str()],
            "expected 2 tab-separated fields",
        ),
        (vec![os("-F"), header.as_os_str()], "header must be exactly"),
        (
            vec![os("-f"), absent.as_os_str()],
            "cannot read --input-list",
        ),
        (
            vec![os("-N"), os("A"), plain.as_os_str(), plain.as_os_str()],
            "exactly one assembly",
        ),
        (
            vec![os("-f"), list.as_os_str(), plain.as_os_str()],
            "cannot be used with",
        ),
        (
            vec![os("-F"), sheet.as_os_str(), plain.as_os_str()],
            "cannot be used with",
        ),
        (
            vec![os("-f"), list.as_os_str(), os("-F"), sheet.as_os_str()],
            "cannot be used with",
        ),
        (
            vec![os("-N"), os("A"), os("-f"), list.as_os_str()],
            "cannot be used with",
        ),
        (
            vec![os("-N"), os("A"), os("-F"), sheet.as_os_str()],
            "cannot be used with",
        ),
        (vec![os("-N"), os("A")], "required"),
        (vec![], "required"),
    ];
    for (index, (mut args, expected)) in cases.into_iter().enumerate() {
        let reports = dir.path().join(format!("reports-{index}"));
        let output = dir.path().join(format!("report-{index}.json"));
        let evidence = dir.path().join(format!("evidence-{index}"));
        args.extend([
            os("-d"),
            reports.as_os_str(),
            os("-o"),
            output.as_os_str(),
            os("-e"),
            evidence.as_os_str(),
        ]);
        let run = ptero(&package, &pin, &args);
        assert_eq!(run.code, Some(2), "{args:?}: {}", run.stderr);
        assert!(run.stderr.contains(expected), "{args:?}: {}", run.stderr);
        assert_eq!(run.report, Value::Null);
        for path in [&reports, &output, &evidence] {
            assert!(!path.exists(), "{args:?} created {}", path.display());
        }
    }
}

#[test]
fn entries_alike_after_dot_dot_removal_but_reaching_different_files_both_run() {
    let (dir, pin, seq) = common::assembly::native_package();
    let base = dir.path().join("base");
    let other = dir.path().join("other");
    std::fs::create_dir_all(other.join("child")).unwrap();
    std::fs::create_dir(&base).unwrap();
    std::os::unix::fs::symlink(other.join("child"), base.join("link")).unwrap();
    let fasta = [b">synthetic\n".as_slice(), &seq, b"\n"].concat();
    std::fs::write(base.join("a.fa"), &fasta).unwrap();
    std::fs::write(
        other.join("a.fa"),
        [fasta.as_slice(), b">short\nACGTN\n"].concat(),
    )
    .unwrap();
    std::os::unix::fs::symlink(base.join("a.fa"), base.join("alias.fa")).unwrap();
    std::fs::write(base.join("list.txt"), "a.fa\nlink/../a.fa\n").unwrap();
    std::fs::write(
        base.join("samples.tsv"),
        "sample\tpath\nbase\ta.fa\nother\tlink/../a.fa\n",
    )
    .unwrap();
    std::fs::write(base.join("aliases.txt"), "a.fa\nalias.fa\n").unwrap();
    for (option, file) in [("-f", "list.txt"), ("-F", "samples.tsv")] {
        let run = ptero(
            &dir.path().join("package"),
            &pin,
            &[os("--no-anim"), os(option), base.join(file).as_os_str()],
        );
        assert_eq!(run.code, Some(0), "{option}: {}", run.stderr);
        let items = run.report.as_array().unwrap();
        assert_eq!(items.len(), 2);
        // Each entry keeps its path as written; that path is the query label.
        assert_eq!(
            items[0]["input"]["file"],
            base.join("a.fa").to_str().unwrap()
        );
        assert_eq!(
            items[1]["input"]["file"],
            base.join("link/../a.fa").to_str().unwrap()
        );
        assert_ne!(
            items[0]["input"]["query_sha256"],
            items[1]["input"]["query_sha256"]
        );
        assert_eq!(items[1]["input"]["total_bases"], 200_008);
    }
    let run = ptero(
        &dir.path().join("package"),
        &pin,
        &[
            os("--no-anim"),
            os("-f"),
            base.join("aliases.txt").as_os_str(),
        ],
    );
    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert!(
        run.stderr.contains("same assembly as line 1"),
        "{}",
        run.stderr
    );
}

#[test]
fn an_output_cannot_replace_a_listed_assembly() {
    let (dir, pin, seq) = common::assembly::native_package();
    let (runs, plain, _) = queries(dir.path(), &seq);
    let list = runs.join("list.txt");
    std::fs::write(&list, "q 1.fa\n").unwrap();
    let before = std::fs::read(&plain).unwrap();
    let run = ptero(
        &dir.path().join("package"),
        &pin,
        &[os("-f"), list.as_os_str(), os("-e"), plain.as_os_str()],
    );
    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert!(run.stderr.contains("is the assembly"), "{}", run.stderr);
    assert_eq!(std::fs::read(&plain).unwrap(), before);
}

#[test]
fn boundary_rescue_from_a_sample_sheet_matches_the_positional_rescue() {
    let (dir, pin, seq) = common::assembly::rescue_package();
    let package = dir.path().join("package");
    let fastas = dir.path().join("fastas");
    let anim_pin = common::package::sha256_hex(&std::fs::read(fastas.join("anim.json")).unwrap());
    let (runs, _, compressed) = queries(dir.path(), &seq);
    let sheet = runs.join("samples.tsv");
    std::fs::write(&sheet, "sample\tpath\nboundary é\tq2 é.fa.gz\n").unwrap();
    let rescue = |source: &[&std::ffi::OsStr], evidence: &str| {
        let evidence = dir.path().join(evidence);
        let mut args = vec![
            os("-a"),
            fastas.as_os_str(),
            os("-A"),
            os(&anim_pin),
            os("-e"),
            evidence.as_os_str(),
        ];
        args.extend_from_slice(source);
        let run = ptero(&package, &pin, &args);
        assert_eq!(run.code, Some(0), "{}", run.stderr);
        assert_eq!(run.report["rescue_status"], "rescued", "{}", run.report);
        run.report
    };
    let positional = rescue(&[compressed.as_os_str()], "positional");
    let sheeted = rescue(&[os("-F"), sheet.as_os_str()], "sheeted");
    assert_eq!(sheeted["input"]["sample_name"], "boundary é");
    assert_eq!(sheeted["input"]["file"], positional["input"]["file"]);
    for key in [
        "species_id",
        "subspecies_id",
        "reasons",
        "counts",
        "call_basis",
        "competitor_evaluation",
        "trigger",
        "trigger_state",
        "legacy_v1_result",
        "top_references",
        "panel_summary",
        "challenge_evidence",
        "evidence",
        "measurement",
    ] {
        assert_eq!(sheeted[key], positional[key], "{key}");
    }
    assert_eq!(
        without_source(&sheeted)["input"],
        without_source(&positional)["input"]
    );
    let pairs = |report: &Value| -> Vec<Value> {
        report["anim_execution"]["pairs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| {
                serde_json::json!([
                    pair["reference_id"],
                    pair["record"]["query"],
                    pair["record"]["orientations"]["QR"]["metrics"],
                    pair["record"]["orientations"]["RQ"]["metrics"]
                ])
            })
            .collect()
    };
    assert_eq!(pairs(&sheeted), pairs(&positional));
    assert_eq!(pairs(&sheeted).len(), 5);
}
