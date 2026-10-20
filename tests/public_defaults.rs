//! Public mode selection is independent of the frozen decision functions.
use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
mod common;

fn command(name: &str) -> assert_cmd::Command {
    let mut command = match name {
        "ptero" => cargo_bin_cmd!("ptero"),
        "pterobacter" => cargo_bin_cmd!("pterobacter"),
        _ => unreachable!(),
    };
    command.arg("--no-mlst");
    command
}

#[test]
fn default_never_falls_back_when_fastas_are_missing() {
    let (dir, pin, seq) = common::assembly::rescue_package();
    let query = dir.path().join("query.fa");
    std::fs::write(&query, [b">query\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    for name in ["ptero", "pterobacter"] {
        let mut results = Vec::new();
        for mode in [vec![], vec!["--threads", "1"]] {
            let output = command(name)
                .args(mode)
                .arg("-r")
                .arg(dir.path().join("package"))
                .args(["-R", &pin])
                .arg(&query)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(1));
            let mut value: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(value["policy_id"], "conservative-assembly-anim-v2");
            assert_eq!(value["trigger"], true);
            assert_eq!(value["species_id"], Value::Null);
            assert_eq!(value["execution"]["status"], "incomplete");
            assert_eq!(value["anim_execution"]["stop_reason"], "missing_component");
            value["anim_execution"]
                .as_object_mut()
                .unwrap()
                .remove("wall_seconds");
            let path = value["data_provenance"]["anim_evidence_directory"]
                .as_str()
                .unwrap();
            assert!(std::path::Path::new(path).is_dir());
            value["data_provenance"]
                .as_object_mut()
                .unwrap()
                .remove("anim_evidence_directory");
            results.push(value);
        }
        assert_eq!(results[0], results[1]);
    }
}

#[test]
fn no_anim_and_its_short_form_agree_for_both_aliases() {
    let (dir, pin, seq) = common::assembly::native_package();
    let query = dir.path().join("query.fa");
    std::fs::write(&query, [b">query\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    let mut results = Vec::new();
    for name in ["ptero", "pterobacter"] {
        for mode in [vec!["--no-anim"], vec!["-n"]] {
            let output = command(name)
                .args(mode)
                .arg("-r")
                .arg(dir.path().join("package"))
                .args(["-R", &pin])
                .arg(&query)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let value: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(value["assignment"]["species_id"], "SYN-SP01");
            assert_eq!(value["versions"]["policy"], "conservative-assembly-v1");
            results.push(value);
        }
    }
    assert!(results.windows(2).all(|w| w[0] == w[1]));
}

#[test]
fn removed_options_and_incompatible_thresholds_are_rejected() {
    let pin = "0".repeat(64);
    for name in ["ptero", "pterobacter"] {
        for flags in [
            vec!["--policy", "conservative-assembly-anim-v2"],
            vec!["-p", "conservative-assembly-v1"],
            vec!["--input-kind", "assembly"],
            vec!["-i", "assembly"],
            vec![
                "--no-anim",
                "--assignment-policy",
                "conservative-assembly-v1",
            ],
            vec!["-n", "-s", "96"],
            vec!["--no-anim", "-S", "98"],
        ] {
            let output = command(name)
                .args(["-r", "missing", "-R", &pin, "missing.fa"])
                .args(&flags)
                .output()
                .unwrap();
            assert!(!output.status.success());
            assert!(
                output.stdout.is_empty(),
                "must reject before reading inputs"
            );
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(
                error.contains("unexpected argument")
                    || error.contains("--no-anim cannot be combined"),
                "{error}"
            );
        }
    }
}

#[test]
fn help_explains_boundary_anim_without_internal_selectors() {
    for name in ["ptero", "pterobacter"] {
        for flag in ["--help", "-h"] {
            let output = command(name).arg(flag).output().unwrap();
            assert!(output.status.success());
            let text = String::from_utf8(output.stdout).unwrap();
            for forbidden in ["policy", "--input-kind", "v1", "v2"] {
                assert!(!text.contains(forbidden), "{text}");
            }
            for expected in [
                "--no-anim",
                "near the species boundary",
                "ANIm can be slower",
                "MLST runs by default",
                "-x/--no-mlst",
                "default: 96",
                "default: 98",
                "skani >=",
                "strictly greater",
                "--species-ani 96.5",
            ] {
                assert!(text.contains(expected), "missing {expected}: {text}");
            }
        }
    }
}
