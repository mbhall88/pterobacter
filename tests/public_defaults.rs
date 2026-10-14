//! Public mode selection is independent of the frozen decision functions.
use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
mod common;

fn command(name: &str) -> assert_cmd::Command {
    match name {
        "tero" => cargo_bin_cmd!("tero"),
        "terobacter" => cargo_bin_cmd!("terobacter"),
        _ => unreachable!(),
    }
}

#[test]
fn default_and_explicit_rescue_agree_and_never_fall_back_when_fastas_are_missing() {
    let (dir, pin, seq) = common::assembly::rescue_package();
    let query = dir.path().join("query.fa");
    std::fs::write(&query, [b">query\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    for name in ["tero", "terobacter"] {
        let mut results = Vec::new();
        for mode in [vec![], vec!["--policy", "conservative-assembly-anim-v2"]] {
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
            results.push(value);
        }
        assert_eq!(results[0], results[1]);
    }
}

#[test]
fn no_anim_and_its_short_form_reproduce_explicit_v1() {
    let (dir, pin, seq) = common::assembly::native_package();
    let query = dir.path().join("query.fa");
    std::fs::write(&query, [b">query\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    let mut results = Vec::new();
    for name in ["tero", "terobacter"] {
        for mode in [
            vec!["--no-anim"],
            vec!["-n"],
            vec!["-p", "conservative-assembly-v1"],
        ] {
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
fn no_anim_rejects_explicit_policy_and_custom_thresholds() {
    let pin = "0".repeat(64);
    for name in ["tero", "terobacter"] {
        for flags in [
            vec!["-n", "--policy", "conservative-assembly-anim-v2"],
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
                error.contains("cannot be used with")
                    || error.contains("v1 rejects threshold overrides"),
                "{error}"
            );
        }
    }
}
