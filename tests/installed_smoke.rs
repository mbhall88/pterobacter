//! Installed-package smoke test: the two public commands from a `cargo install` root classify a
//! synthetic assembly under v1 and v2, offline, with no development binary installed.
//!
//! `just install-check` installs into a temporary root and runs this test with
//! `TEROBACTER_INSTALLED_BIN` naming the install's `bin` directory. It is ignored otherwise,
//! because an ordinary test build has no installed package to check.
mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn installed_bin() -> PathBuf {
    let dir = std::env::var_os("TEROBACTER_INSTALLED_BIN")
        .expect("set TEROBACTER_INSTALLED_BIN to the bin directory of a cargo install root");
    PathBuf::from(dir)
}

/// Run an installed command with an empty environment, so nothing on PATH can stand in for it.
fn run(bin: &Path, name: &str, args: &[&std::ffi::OsStr]) -> Output {
    Command::new(bin.join(name))
        .env_clear()
        .args(args)
        .output()
        .unwrap()
}

fn report(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{error}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
#[ignore = "needs an installed package; run by just install-check"]
fn installed_commands_classify_identically_under_v1_and_v2() {
    let bin = installed_bin();
    let mut installed: Vec<String> = std::fs::read_dir(&bin)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    installed.sort();
    assert_eq!(installed, ["tero", "terobacter"]);

    let version = format!("terobacter {}\n", env!("CARGO_PKG_VERSION"));
    for name in ["tero", "terobacter"] {
        let output = run(&bin, name, &["--version".as_ref()]);
        assert!(output.status.success(), "{name} --version");
        assert_eq!(String::from_utf8_lossy(&output.stdout), version, "{name}");
    }

    // v1: an assigned call, identical byte for byte from both names.
    let (dir, pin, seq) = common::assembly::native_package();
    let query = dir.path().join("query.fa");
    std::fs::write(&query, [b">synthetic\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    let package = dir.path().join("package");
    let mut v1 = Vec::new();
    for name in ["tero", "terobacter"] {
        let reports = dir.path().join(format!("{name}-v1"));
        let output = run(
            &bin,
            name,
            &[
                "--reference-package".as_ref(),
                package.as_os_str(),
                "--expected-sha256".as_ref(),
                pin.as_ref(),
                "--report-dir".as_ref(),
                reports.as_os_str(),
                query.as_os_str(),
            ],
        );
        assert!(output.status.success(), "{name}: {output:?}");
        let value = report(&output);
        assert_eq!(value["execution"]["status"], "complete");
        assert_eq!(value["assignment"]["status"], "assigned");
        assert_eq!(value["assignment"]["species_id"], "SYN-SP01");
        assert_eq!(
            value["implementation"]["version"],
            env!("CARGO_PKG_VERSION")
        );
        for file in ["report.json", "calls.tsv", "evidence.tsv", "report.md"] {
            assert!(reports.join(file).is_file(), "{name}: {file}");
        }
        v1.push(value);
    }
    assert_eq!(v1[0], v1[1]);

    // v2: a native ANIm rescue through the embedded aligner, with no executable on PATH.
    let (dir, pin, seq) = common::assembly::rescue_package();
    let query = dir.path().join("query.fa");
    std::fs::write(&query, [b">synthetic\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    let anim_pin =
        common::package::sha256_hex(&std::fs::read(dir.path().join("fastas/anim.json")).unwrap());
    let mut v2 = Vec::new();
    for name in ["tero", "terobacter"] {
        let receipts = dir.path().join(format!("{name}-anim"));
        let output = run(
            &bin,
            name,
            &[
                "--reference-package".as_ref(),
                dir.path().join("package").as_os_str(),
                "--expected-sha256".as_ref(),
                pin.as_ref(),
                "--policy".as_ref(),
                "conservative-assembly-anim-v2".as_ref(),
                "--anim-reference-package".as_ref(),
                dir.path().join("fastas").as_os_str(),
                "--expected-anim-sha256".as_ref(),
                anim_pin.as_ref(),
                "--anim-output".as_ref(),
                receipts.as_os_str(),
                query.as_os_str(),
            ],
        );
        assert!(output.status.success(), "{name}: {output:?}");
        let value = report(&output);
        assert_eq!(value["execution"]["status"], "complete");
        assert_eq!(value["rescue_status"], "rescued");
        assert_eq!(value["call_basis"], "ANIm_assisted");
        assert_eq!(value["validation_status"], "not_yet_validated");
        v2.push(value);
    }
    // Timings and receipt directories differ between runs; the decision does not.
    for field in [
        "status",
        "species_id",
        "subspecies_id",
        "reasons",
        "trigger",
        "trigger_state",
        "call_basis",
        "competitor_evaluation",
        "rescue_status",
        "counts",
        "legacy_v1_result",
        "versions",
    ] {
        assert_eq!(v2[0][field], v2[1][field], "{field}");
    }
}
