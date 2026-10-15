//! Command-line seam: `ptero` and `pterobacter` are two names for one implementation.

use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;

const COMMANDS: [&str; 2] = ["ptero", "pterobacter"];

fn command(name: &str) -> Command {
    match name {
        "ptero" => cargo_bin_cmd!("ptero"),
        "pterobacter" => cargo_bin_cmd!("pterobacter"),
        other => panic!("unknown command {other}"),
    }
}

struct Observed {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// Run `name` with `args`, hiding only the command name in usage lines so the two can be compared.
fn observe(name: &str, args: &[&str]) -> Observed {
    let output = command(name)
        .args(args)
        .output()
        .expect("command should run");
    let hide_name = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes)
            .replace(&format!("Usage: {name}"), "Usage: <command>")
            .replace(&format!("       {name} "), "       <command> ")
    };
    Observed {
        code: output.status.code(),
        stdout: hide_name(&output.stdout),
        stderr: hide_name(&output.stderr),
    }
}

#[test]
fn version_names_terobacter_and_its_implementation_version() {
    for name in COMMANDS {
        let output = command(name).arg("--version").output().unwrap();

        assert!(output.status.success(), "{name} --version failed");
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("pterobacter {}\n", env!("CARGO_PKG_VERSION")),
            "{name} should name Pterobacter, not the command alias"
        );
    }
}

#[test]
fn both_commands_give_the_same_output_and_exit_status() {
    let cases: [&[&str]; 6] = [
        &["--version"],
        &["-V"],
        &["--help"],
        &["-h"],
        &[],
        &["no-such-assembly.fa"],
    ];
    for args in cases {
        let tero = observe("ptero", args);
        let terobacter = observe("pterobacter", args);

        assert_eq!(
            tero.code, terobacter.code,
            "exit status differs for {args:?}"
        );
        assert_eq!(
            tero.stdout, terobacter.stdout,
            "stdout differs for {args:?}"
        );
        assert_eq!(
            tero.stderr, terobacter.stderr,
            "stderr differs for {args:?}"
        );
    }
}

#[test]
fn help_shows_the_approved_classification_and_database_interfaces() {
    let output = command("ptero").arg("--help").output().unwrap();
    let help = String::from_utf8(output.stdout).unwrap();

    assert!(
        help.contains("Usage: ptero [OPTIONS] <ASSEMBLY>..."),
        "{help}"
    );
    assert!(
        help.contains("Commands:") && help.contains("database"),
        "the public command exposes managed database operations:\n{help}"
    );
}

#[test]
fn an_assembly_run_without_a_pinned_default_requires_an_explicit_package_or_version() {
    for name in COMMANDS {
        let output = command(name).arg("query.fa").output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("--reference-package"), "{stderr}");
        assert!(stderr.contains("--expected-sha256"), "{stderr}");
    }
}

#[test]
fn package_name_matches_the_public_identity() {
    assert_eq!(env!("CARGO_PKG_NAME"), "pterobacter");
}
