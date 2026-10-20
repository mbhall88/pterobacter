//! Synthetic end-to-end thresholds through the public native assembly workflow.
mod common;

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
use terobacter::genomovar::GenomovarParent;
use terobacter::rescue::Thresholds;

fn command(name: &str) -> assert_cmd::Command {
    let mut command = match name {
        "ptero" => cargo_bin_cmd!("ptero"),
        "pterobacter" => cargo_bin_cmd!("pterobacter"),
        _ => unreachable!(),
    };
    command.arg("--no-mlst");
    command
}
fn run(
    name: &str,
    dir: &std::path::Path,
    pin: &str,
    query: &std::path::Path,
    options: &[&str],
) -> (Option<i32>, Value) {
    let output = command(name)
        .arg("--reference-package")
        .arg(dir.join("package"))
        .args(["--expected-sha256", pin])
        .args(options)
        .arg(query)
        .output()
        .unwrap();
    let report = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(&output.stderr)));
    (output.status.code(), report)
}
fn assert_policy(report: &Value, thresholds: Thresholds) {
    for (key, value) in thresholds.policy_value().as_object().unwrap() {
        assert_eq!(&report[key], value, "{key}");
    }
}

#[test]
fn explicit_defaults_keep_native_calls_but_mark_custom_and_preserve_literals() {
    let (dir, pin, seq) = common::assembly::native_package();
    let query = dir.path().join("query.fa");
    std::fs::write(&query, [b">query\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    let (code, default) = run("ptero", dir.path(), &pin, &query, &[]);
    assert_eq!(code, Some(0));
    assert_policy(&default, Thresholds::standard());
    for name in ["ptero", "pterobacter"] {
        for flags in [
            vec!["--species-ani", "96"],
            vec!["--subspecies-ani", "98"],
            vec!["--species-ani", "96.000", "--subspecies-ani", "98.0"],
        ] {
            let (code, report) = run(name, dir.path(), &pin, &query, &flags);
            assert_eq!(code, Some(0));
            assert_eq!(report["policy_mode"], "custom");
            assert_eq!(report["validation_status"], "custom_unvalidated");
            assert_ne!(report["settings_sha256"], default["settings_sha256"]);
            for key in [
                "species_id",
                "subspecies_id",
                "call_basis",
                "legacy_v1_result",
                "trigger",
                "reasons",
            ] {
                assert_eq!(report[key], default[key], "{key}");
            }
            assert!(!GenomovarParent::from_report(&report).verified);
            assert_eq!(report["genomovar"]["search"]["status"], "not_run");
            assert_eq!(report["genomovar_id"], Value::Null);
            if flags.contains(&"96.000") {
                assert_policy(
                    &report,
                    Thresholds::custom(Some("96.000"), Some("98.0")).unwrap(),
                );
            }
        }
    }
}

#[test]
fn native_skani_equality_and_long_decimal_boundary_change_species_and_subspecies() {
    let (dir, pin, seq) = common::assembly::rescue_package();
    let query = dir.path().join("query.fa");
    std::fs::write(&query, [b">query\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    let (_, baseline) = run("ptero", dir.path(), &pin, &query, &[]);
    let measured = baseline["max_skani_ani_percent"].to_string();
    // A threshold equal to the emitted hundredths takes the ordinary skani route.
    let (code, equal) = run(
        "ptero",
        dir.path(),
        &pin,
        &query,
        &["--species-ani", &measured, "--subspecies-ani", &measured],
    );
    assert_eq!(code, Some(0), "{equal}");
    assert_eq!(equal["trigger"], false);
    assert_eq!(equal["species_id"], "SYN-SP01");
    assert_eq!(equal["subspecies_id"], "SYN-SS01");
    assert_eq!(equal["call_basis"], "skani");
    assert_eq!(equal["legacy_v1_result"]["species_id"], Value::Null);
    let above = format!(
        "{measured}{}1",
        if measured.contains('.') {
            "0".repeat(24)
        } else {
            format!(".{}", "0".repeat(24))
        }
    );
    // Raising S alone withholds subspecies without changing the species call.
    let (code, child) = run(
        "ptero",
        dir.path(),
        &pin,
        &query,
        &["--species-ani", &measured, "--subspecies-ani", &above],
    );
    assert_eq!(code, Some(0));
    assert_eq!(child["species_id"], "SYN-SP01");
    assert_eq!(child["subspecies_id"], Value::Null);
    // Raising T by a decimal smaller than f64 precision enters the rescue band.
    let (code, raised) = run(
        "ptero",
        dir.path(),
        &pin,
        &query,
        &["--species-ani", &above],
    );
    assert_ne!(code, Some(0));
    assert_eq!(raised["trigger"], true);
    assert_eq!(raised["anim_plan"]["stage"], "witness");
    assert_eq!(raised["species_id"], Value::Null);
    assert_policy(&raised, Thresholds::custom(Some(&above), None).unwrap());
    assert_eq!(raised["anim_execution"]["stop_reason"], "missing_component");
}

#[test]
fn premeasurement_failures_retain_custom_provenance_and_unknown_policy_runs_nothing() {
    let (dir, pin, seq) = common::assembly::native_package();
    let query = dir.path().join("query.fa");
    std::fs::write(&query, [b">query\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    let options = ["--species-ani", "96.000"];
    for (query, hash, code, stage) in [
        (dir.path().join("missing.fa"), pin.clone(), 3, "input"),
        (query.clone(), "0".repeat(64), 4, "package"),
    ] {
        let (actual, report) = run("ptero", dir.path(), &hash, &query, &options);
        assert_eq!(actual, Some(code));
        assert_eq!(report["execution"]["stage"], stage);
        assert_policy(&report, Thresholds::custom(Some("96.000"), None).unwrap());
    }
    let existing = dir.path().join("existing");
    std::fs::create_dir(&existing).unwrap();
}

#[test]
fn v1_and_invalid_custom_flags_are_rejected_before_inputs_are_read() {
    for name in ["ptero", "pterobacter"] {
        for flags in [
            vec!["--no-anim", "--species-ani", "96"],
            vec!["--no-anim", "--subspecies-ani", "98"],
            vec!["--species-ani", "79.99"],
            vec!["--species-ani", "NaN"],
            vec!["--subspecies-ani", "95"],
        ] {
            let output = command(name)
                .args([
                    "--reference-package",
                    "missing",
                    "--expected-sha256",
                    &"0".repeat(64),
                ])
                .args(flags)
                .arg("missing.fa")
                .output()
                .unwrap();
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(
                error.contains("--no-anim cannot be combined")
                    || error.contains("80 <=")
                    || error.contains("finite decimal"),
                "{error}"
            );
        }
    }
}

#[test]
fn custom_native_anim_rescue_uses_selected_threshold_and_keeps_species_only() {
    let (dir, pin, seq) = common::assembly::rescue_package();
    let query = dir.path().join("query.fa");
    std::fs::write(&query, [b">query\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    let anim_pin =
        common::package::sha256_hex(&std::fs::read(dir.path().join("fastas/anim.json")).unwrap());
    for (threshold, expected) in [("95.9", "rescued"), ("97.1", "not_triggered")] {
        let receipts = dir.path().join(format!("receipts-{threshold}"));
        let output = command("ptero")
            .arg("--reference-package")
            .arg(dir.path().join("package"))
            .args([
                "--expected-sha256",
                &pin,
                "--species-ani",
                threshold,
                "--expected-anim-sha256",
                &anim_pin,
            ])
            .arg("--anim-reference-package")
            .arg(dir.path().join("fastas"))
            .arg("--anim-output")
            .arg(receipts)
            .arg(&query)
            .output()
            .unwrap();
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(output.status.success(), "{report}");
        assert_policy(&report, Thresholds::custom(Some(threshold), None).unwrap());
        assert_eq!(report["trigger"], expected == "rescued", "{report}");
        assert_eq!(report["rescue_status"], expected, "{report}");
        assert_eq!(report["subspecies_id"], Value::Null);
        assert_eq!(report["genomovar_id"], Value::Null);
        assert_eq!(report["genomovar"]["search"]["status"], "not_run");
        if expected == "rescued" {
            assert_eq!(report["species_id"], "SYN-SP01");
            assert_eq!(report["call_basis"], "ANIm_assisted");
            assert_eq!(report["counts"]["completed"], 5);
        } else {
            assert_eq!(report["species_id"], Value::Null);
            assert_eq!(report["counts"]["attempted"], 0);
            assert_eq!(report["anim_plan"]["stage"], "not_triggered");
        }
    }
}
