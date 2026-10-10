//! `--report-dir` writes the JSON report and its TSV and Markdown renderings together.
use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
mod common;
use common::assembly::native_package;

fn tero(
    dir: &std::path::Path,
    pin: &str,
    extra: &[&std::ffi::OsStr],
    query: &[&std::path::Path],
) -> std::process::Output {
    let mut cmd = cargo_bin_cmd!("tero");
    cmd.arg("--reference-package")
        .arg(dir.join("package"))
        .args(["--expected-sha256", pin]);
    cmd.args(extra);
    cmd.args(query);
    cmd.output().unwrap()
}

fn column(tsv: &str, row: usize, name: &str) -> String {
    let lines: Vec<Vec<&str>> = tsv.lines().map(|l| l.split('\t').collect()).collect();
    let at = lines[0].iter().position(|c| *c == name).unwrap();
    lines[row + 1][at].to_owned()
}

#[test]
fn report_dir_holds_four_agreeing_files_and_stdout_stays_the_json_report() {
    let (dir, pin, seq) = native_package();
    let query = dir.path().join("query.fa");
    std::fs::write(
        &query,
        [b">chr\n".as_slice(), &seq, b"\n>plasmid\nACGTN\n"].concat(),
    )
    .unwrap();
    let reports = dir.path().join("reports");
    let output = tero(
        dir.path(),
        &pin,
        &["--report-dir".as_ref(), reports.as_os_str()],
        &[&query],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
    let on_disk: Value =
        serde_json::from_slice(&std::fs::read(reports.join("report.json")).unwrap()).unwrap();
    assert_eq!(stdout, on_disk);
    let mut names: Vec<String> = std::fs::read_dir(&reports)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["calls.tsv", "evidence.tsv", "report.json", "report.md"]
    );

    let calls = std::fs::read_to_string(reports.join("calls.tsv")).unwrap();
    assert_eq!(
        column(&calls, 0, "species_id"),
        stdout["assignment"]["species_id"]
    );
    assert_eq!(column(&calls, 0, "call_status"), "assigned");
    assert_eq!(
        column(&calls, 0, "query_sha256"),
        stdout["input"]["query_sha256"]
    );
    assert_eq!(column(&calls, 0, "min_species_ani"), "96");
    assert_eq!(column(&calls, 0, "package_manifest_sha256"), pin);
    let markdown = std::fs::read_to_string(reports.join("report.md")).unwrap();
    assert!(markdown.contains("Enterobacter syntheticus (SYN-SP01)"));
    let evidence = std::fs::read_to_string(reports.join("evidence.tsv")).unwrap();
    assert!(evidence.lines().any(|l| l.contains("\ttop_reference\t")));
    assert!(evidence.lines().any(|l| l.contains("\tpanel_parent\t")));
}

#[test]
fn v1_json_carries_the_strongest_matches_panel_summary_and_distinct_versions() {
    let (dir, pin, seq) = native_package();
    let query = dir.path().join("query.fa");
    std::fs::write(&query, [b">chr\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    let output = tero(dir.path(), &pin, &[], &[&query]);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let top = report["top_references"].as_array().unwrap();
    assert_eq!(top.len(), 5, "five references in the synthetic panel");
    assert_eq!(top[0]["tie_group"], 0);
    assert_eq!(top[0]["anim"], Value::Null, "v1 runs no ANIm");
    let panel = report["panel_summary"].as_array().unwrap();
    assert_eq!(panel.len(), 2);
    let reference_total: u64 = panel
        .iter()
        .map(|p| p["panel_reference_count"].as_u64().unwrap())
        .sum();
    assert_eq!(
        reference_total, 5,
        "every panel reference sits in a parent row"
    );
    assert_eq!(report["thresholds"]["species_ani"], "96");
    assert_eq!(report["thresholds"]["subspecies_ani"], "98");
    assert_eq!(report["thresholds"]["both_af_minimum"], "80");
    assert_eq!(
        report["thresholds"]["subspecies_margin_percentage_points"],
        "1"
    );
    let versions = &report["versions"];
    assert_eq!(versions["policy"], "conservative-assembly-v1");
    assert_eq!(versions["policy_contract"], "frozen-0.2.0");
    assert_eq!(versions["report_schema"], "terobacter.assembly-call/1");
    assert_eq!(
        versions["skani_method"],
        "skani-0.3.2-avx2-compatible-dna-v1"
    );
    assert_eq!(versions["implementation"], env!("CARGO_PKG_VERSION"));
    assert_eq!(versions["reference_data"]["manifest_sha256"], pin);
}

#[test]
fn an_existing_report_dir_is_refused_before_any_analysis_and_left_unchanged() {
    let (dir, pin, seq) = native_package();
    let query = dir.path().join("query.fa");
    std::fs::write(&query, [b">chr\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    let reports = dir.path().join("reports");
    std::fs::create_dir(&reports).unwrap();
    std::fs::write(reports.join("report.json"), b"previous").unwrap();
    let output = tero(
        dir.path(),
        &pin,
        &["--report-dir".as_ref(), reports.as_os_str()],
        &[&query],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty(), "no analysis output");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--report-dir must be a new directory")
    );
    assert_eq!(
        std::fs::read(reports.join("report.json")).unwrap(),
        b"previous"
    );
    assert_eq!(std::fs::read_dir(&reports).unwrap().count(), 1);
}

#[test]
fn a_failed_input_still_writes_every_report_with_an_incomplete_row() {
    let (dir, pin, seq) = native_package();
    let good = dir.path().join("good.fa");
    std::fs::write(&good, [b">chr\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    let bad = dir.path().join("bad.fa");
    std::fs::write(&bad, b"@reads\nACGT\n+\n!!!!\n").unwrap();
    let reports = dir.path().join("reports");
    let output = tero(
        dir.path(),
        &pin,
        &["--report-dir".as_ref(), reports.as_os_str()],
        &[&good, &bad],
    );
    assert_eq!(output.status.code(), Some(3));
    let calls = std::fs::read_to_string(reports.join("calls.tsv")).unwrap();
    assert_eq!(calls.lines().count(), 3);
    assert_eq!(column(&calls, 0, "call_status"), "assigned");
    assert_eq!(column(&calls, 1, "call_status"), "not_evaluated");
    assert_eq!(column(&calls, 1, "execution_status"), "incomplete");
    assert_eq!(column(&calls, 1, "execution_stage"), "input");
    assert_eq!(column(&calls, 1, "species_id"), "");
    let json: Value =
        serde_json::from_slice(&std::fs::read(reports.join("report.json")).unwrap()).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 2);
    let markdown = std::fs::read_to_string(reports.join("report.md")).unwrap();
    assert!(markdown.contains("Assembly 2"));
    assert!(markdown.contains("did not complete"));
}

/// Run the v2 policy with a fresh ANIm component and return the JSON report and the report dir.
fn v2_run(case: common::assembly::NativeCase) -> (tempfile::TempDir, Value) {
    let (dir, pin, seq) = common::assembly::native_case(case);
    let query = dir.path().join("query.fa");
    std::fs::write(
        &query,
        [b">chr\n".as_slice(), &seq, b"\n>short\nACGTN\n"].concat(),
    )
    .unwrap();
    let anim_pin =
        common::package::sha256_hex(&std::fs::read(dir.path().join("fastas/anim.json")).unwrap());
    let reports = dir.path().join("reports");
    let receipts = dir.path().join("receipts");
    let fastas = dir.path().join("fastas");
    let output = tero(
        dir.path(),
        &pin,
        &[
            "--policy".as_ref(),
            "conservative-assembly-anim-v2".as_ref(),
            "--expected-anim-sha256".as_ref(),
            anim_pin.as_ref(),
            "--anim-reference-package".as_ref(),
            fastas.as_os_str(),
            "--anim-output".as_ref(),
            receipts.as_os_str(),
            "--report-dir".as_ref(),
            reports.as_os_str(),
        ],
        &[&query],
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    (dir, json)
}

#[test]
fn a_native_rescue_agrees_across_formats_including_anim_counts_and_methods() {
    let (dir, json) = v2_run(common::assembly::NativeCase::Rescue);
    assert_eq!(json["rescue_status"], "rescued", "{json}");
    let reports = dir.path().join("reports");
    let calls = std::fs::read_to_string(reports.join("calls.tsv")).unwrap();
    for (name, key) in [
        ("anim_required", "required"),
        ("anim_attempted", "attempted"),
        ("anim_completed", "completed"),
        ("anim_no_alignment", "no_alignment"),
        ("anim_failed", "failed"),
        ("anim_not_run", "not_run"),
    ] {
        assert_eq!(
            column(&calls, 0, name),
            json["counts"][key].to_string(),
            "{name}"
        );
    }
    assert_eq!(column(&calls, 0, "call_status"), "assigned");
    assert_eq!(column(&calls, 0, "species_id"), json["species_id"]);
    assert_eq!(column(&calls, 0, "call_basis"), "ANIm_assisted");
    assert_eq!(column(&calls, 0, "validation_status"), "not_yet_validated");
    assert_eq!(
        column(&calls, 0, "anim_method"),
        json["anim_execution"]["method_profile"]
    );
    assert_eq!(
        column(&calls, 0, "original_v1_status"),
        json["legacy_v1_result"]["status"]
    );
    assert_eq!(column(&calls, 0, "trigger"), "true");
    assert_eq!(json["versions"]["validation"], "not_yet_validated");
    assert_eq!(
        json["versions"]["anim_method"],
        json["anim_execution"]["method_profile"]
    );
    let markdown = std::fs::read_to_string(reports.join("report.md")).unwrap();
    assert!(markdown.contains("ANIm pairs: 5 required, 5 attempted, 5 completed, 1 no alignment"));
    assert!(markdown.contains("Original v1 outcome: unresolved"));
}

#[test]
fn a_native_foreign_challenge_withholds_the_label_in_every_format() {
    let (dir, json) = v2_run(common::assembly::NativeCase::ForeignChallenge);
    assert_eq!(json["status"], "unresolved", "{json}");
    let reports = dir.path().join("reports");
    let calls = std::fs::read_to_string(reports.join("calls.tsv")).unwrap();
    for name in [
        "species_id",
        "species_name",
        "subspecies_id",
        "genomovar_id",
    ] {
        assert_eq!(column(&calls, 0, name), "", "{name}");
    }
    assert_eq!(column(&calls, 0, "call_status"), "unresolved");
    assert!(column(&calls, 0, "reasons").contains("foreign_parent_challenge"));
    let challenge = json["challenges"][0].as_str().unwrap();
    let evidence = std::fs::read_to_string(reports.join("evidence.tsv")).unwrap();
    assert!(evidence
        .lines()
        .any(|l| l.contains("\tchallenge\t") && l.contains(challenge)));
    let markdown = std::fs::read_to_string(reports.join("report.md")).unwrap();
    assert!(markdown.contains(challenge));
    assert!(markdown.contains("**Species:** unresolved"));
    assert!(!markdown.contains("assigned —"));
}

#[test]
fn custom_v2_thresholds_are_custom_unvalidated_in_the_json_and_every_rendering() {
    let (dir, pin, seq) = native_package();
    let query = dir.path().join("query.fa");
    std::fs::write(&query, [b">chr\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    let reports = dir.path().join("reports");
    let output = tero(
        dir.path(),
        &pin,
        &[
            "--policy".as_ref(),
            "conservative-assembly-anim-v2".as_ref(),
            "--species-ani".as_ref(),
            "96".as_ref(),
            "--subspecies-ani".as_ref(),
            "98.5".as_ref(),
            "--report-dir".as_ref(),
            reports.as_os_str(),
        ],
        &[&query],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json["validation_status"], "custom_unvalidated",
        "default values are still custom"
    );
    let calls = std::fs::read_to_string(reports.join("calls.tsv")).unwrap();
    assert_eq!(column(&calls, 0, "policy_mode"), "custom");
    assert_eq!(column(&calls, 0, "validation_status"), "custom_unvalidated");
    assert_eq!(
        column(&calls, 0, "min_species_ani"),
        json["thresholds"]["species_ani"]
    );
    assert_eq!(column(&calls, 0, "min_subspecies_ani"), "98.5");
    assert_eq!(json["versions"]["validation"], "custom_unvalidated");
    let markdown = std::fs::read_to_string(reports.join("report.md")).unwrap();
    assert!(markdown.contains("custom_unvalidated"));
}

#[test]
fn a_report_write_failure_still_prints_the_json_and_exits_nonzero() {
    use std::io::Write;
    use std::os::unix::ffi::OsStrExt;
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    let (dir, pin, seq) = native_package();
    // A FIFO query pauses the run after the report directory exists and before it writes.
    let query = dir.path().join("query.fa");
    let name = std::ffi::CString::new(query.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let reports = dir.path().join("reports");
    let mut cmd = cargo_bin_cmd!("tero");
    cmd.arg("--reference-package")
        .arg(dir.path().join("package"))
        .args(["--expected-sha256", &pin, "--report-dir"])
        .arg(&reports)
        .arg(&query);
    let child = std::process::Command::new(cmd.get_program())
        .args(cmd.get_args())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while !reports.is_dir() && start.elapsed() < Duration::from_secs(20) {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(reports.is_dir(), "the run creates the directory first");
    // The earlier file is kept and makes the final write fail.
    std::fs::write(reports.join("report.json"), b"earlier").unwrap();
    let mut fifo = std::fs::OpenOptions::new()
        .write(true)
        .open(&query)
        .unwrap();
    fifo.write_all(&[b">chr\n".as_slice(), &seq, b"\n"].concat())
        .unwrap();
    drop(fifo);
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("error writing reports"));
    let report: Value = serde_json::from_slice(&output.stdout)
        .expect("the current report stays on stdout when the files cannot be written");
    assert_eq!(report["assignment"]["status"], "assigned");
    assert_eq!(
        std::fs::read(reports.join("report.json")).unwrap(),
        b"earlier",
        "an existing file is never replaced"
    );
}

#[test]
fn a_report_dir_inside_an_input_package_is_refused_before_any_analysis() {
    let (dir, pin, seq) = native_package();
    let query = dir.path().join("query.fa");
    std::fs::write(&query, [b">chr\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    let inside = dir.path().join("package").join("reports");
    let output = tero(
        dir.path(),
        &pin,
        &["--report-dir".as_ref(), inside.as_os_str()],
        &[&query],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("inside the input package"));
    assert!(!inside.exists(), "nothing is created inside the package");
}

#[test]
fn unmeasured_challenges_are_not_reported_as_negative_evidence() {
    let (dir, pin, _) = native_package();
    let missing = dir.path().join("missing.fa");
    let reports = dir.path().join("reports");
    let output = std::process::Command::new(cargo_bin_cmd!("tero").get_program())
        .arg("--reference-package")
        .arg(dir.path().join("package"))
        .args([
            "--expected-sha256",
            &pin,
            "--policy",
            "conservative-assembly-anim-v2",
        ])
        .arg("--report-dir")
        .arg(&reports)
        .arg(&missing)
        .output()
        .unwrap();
    assert_ne!(output.status.code(), Some(0));
    let markdown = std::fs::read_to_string(reports.join("report.md")).unwrap();
    assert!(
        !markdown.contains("No reference challenged the call"),
        "{markdown}"
    );
    assert!(markdown.contains("Not assessed"), "{markdown}");
    let calls = std::fs::read_to_string(reports.join("calls.tsv")).unwrap();
    assert_eq!(column(&calls, 0, "challenge_assessment"), "not_assessed");
}
