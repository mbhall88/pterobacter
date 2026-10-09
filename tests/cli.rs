//! Command-line seam: `tero` and `terobacter` are two names for one implementation.

use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;

const COMMANDS: [&str; 2] = ["tero", "terobacter"];

fn command(name: &str) -> Command {
    match name {
        "tero" => cargo_bin_cmd!("tero"),
        "terobacter" => cargo_bin_cmd!("terobacter"),
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
        String::from_utf8_lossy(bytes).replace(&format!("Usage: {name}"), "Usage: <command>")
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
            format!("terobacter {}\n", env!("CARGO_PKG_VERSION")),
            "{name} should name Terobacter, not the command alias"
        );
    }
}

#[test]
fn both_commands_give_the_same_output_and_exit_status() {
    let cases: [&[&str]; 4] = [&["--version"], &["--help"], &[], &["no-such-assembly.fa"]];
    for args in cases {
        let tero = observe("tero", args);
        let terobacter = observe("terobacter", args);

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
fn help_shows_the_approved_interface_without_subcommands() {
    let output = command("tero").arg("--help").output().unwrap();
    let help = String::from_utf8(output.stdout).unwrap();

    assert!(
        help.contains("Usage: tero [OPTIONS] <ASSEMBLY>..."),
        "{help}"
    );
    assert!(
        !help.contains("Commands:"),
        "the public command has no subcommands:\n{help}"
    );
}

#[test]
fn an_assembly_run_fails_loudly_until_classification_exists() {
    for name in COMMANDS {
        let output = command(name).arg("query.fa").output().unwrap();

        assert_eq!(
            output.status.code(),
            Some(1),
            "{name} must not look successful"
        );
        assert!(
            output.stdout.is_empty(),
            "{name} printed a result it did not compute"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("not implemented"),
            "{name} should say why it stopped"
        );
    }
}

#[test]
fn both_public_commands_validate_custom_settings_before_the_assembly_stub() {
    for name in COMMANDS {
        for flag in ["--species-ani=96", "--subspecies-ani=98"] {
            let out = command(name).args([flag, "query.fa"]).output().unwrap();
            assert!(!out.status.success());
            assert!(out.stdout.is_empty());
            assert!(String::from_utf8_lossy(&out.stderr).contains("v1 rejects threshold overrides"));
        }
        let out = command(name)
            .args([
                "--assignment-policy",
                "conservative-assembly-anim-v2",
                "--species-ani",
                "79.99",
                "query.fa",
            ])
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("80 <="));
        let out = command(name)
            .args([
                "--assignment-policy",
                "conservative-assembly-anim-v2",
                "--species-ani",
                "96",
                "query.fa",
            ])
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(out.stdout.is_empty());
        assert!(String::from_utf8_lossy(&out.stderr).contains("not implemented"));
    }
}
