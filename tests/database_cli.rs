//! Public managed commands and native rescue, using existing synthetic derivations.
mod common;
use common::database::Fixture;
use serde_json::Value;
use std::{fs, path::Path};
trait FixtureCommand {
    fn command(&self, store: &Path) -> assert_cmd::Command;
}
impl FixtureCommand for Fixture {
    fn command(&self, store: &Path) -> assert_cmd::Command {
        let mut c = assert_cmd::cargo::cargo_bin_cmd!("ptero");
        c.args([
            "--catalogue",
            self.catalogue_path.to_str().unwrap(),
            "--catalogue-sha256",
            &self.catalogue_pin,
            "--database-dir",
            store.to_str().unwrap(),
        ]);
        c.env("XDG_STATE_HOME", self.source.path().join("state"));
        c.env("HOME", self.source.path().join("home"));
        c
    }
}
fn value(out: std::process::Output) -> Value {
    assert!(
        out.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
fn install(f: &Fixture, store: &Path, extra: &[&str]) -> Value {
    value(
        f.command(store)
            .args([
                "database",
                "install",
                "fixture-1",
                "--from-directory",
                f.archive_dir.to_str().unwrap(),
            ])
            .args(extra)
            .output()
            .unwrap(),
    )
}
#[test]
fn installs_then_adds_optional_typing_and_reuses_offline() {
    let f = Fixture::new();
    let store = f.source.path().join("store");
    install(&f, &store, &[]);
    let dir = store.join("synthetic/fixture-1");
    assert!(dir.join("primary-fasta/anim.json").is_file());
    assert!(!dir.join("mlst-historical").exists());
    let before = fs::read(dir.join("reference/portable.json")).unwrap();
    install(&f, &store, &["--mlst"]);
    assert_eq!(
        before,
        fs::read(dir.join("reference/portable.json")).unwrap()
    );
    assert!(dir.join("mlst-historical/manifest.json").is_file());
    fs::remove_dir_all(&f.archive_dir).unwrap();
    let reuse = value(
        f.command(&store)
            .args(["database", "install", "fixture-1", "--mlst"])
            .output()
            .unwrap(),
    );
    assert!(reuse["components"]
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c["status"] == "verified_reuse"));
    value(
        f.command(&store)
            .args(["database", "verify", "fixture-1"])
            .output()
            .unwrap(),
    );
    fs::write(dir.join("primary-fasta/sequences/0.fa"), b"corrupt").unwrap();
    let out = f
        .command(&store)
        .args(["database", "verify", "fixture-1"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(
        before,
        fs::read(dir.join("reference/portable.json")).unwrap()
    );
}
#[test]
fn managed_native_rescue_matches_explicit_invocation_and_retains_unique_evidence() {
    let f = Fixture::new();
    let store = f.source.path().join("store");
    install(&f, &store, &[]);
    let missing_typing = f
        .command(&store)
        .args(["--database", "fixture-1", "--mlst"])
        .arg(&f.query)
        .output()
        .unwrap();
    assert_eq!(missing_typing.status.code(), Some(6));
    let managed: Value = serde_json::from_slice(&missing_typing.stdout).unwrap();
    assert_eq!(managed["mlst"]["status"], "failed");
    assert_eq!(managed["call_basis"], "ANIm_assisted");
    assert!(managed["counts"]["completed"].as_u64().unwrap() > 0);
    let evidence = managed["data_provenance"]["anim_evidence_directory"]
        .as_str()
        .unwrap();
    assert!(Path::new(evidence).is_dir());
    assert!(Path::new(evidence).join("query-000001").is_dir());
    let explicit = value(
        assert_cmd::cargo::cargo_bin_cmd!("ptero")
            .args([
                "--reference-package",
                f.source.path().join("package").to_str().unwrap(),
                "--expected-sha256",
                &f.reference,
                "--anim-reference-package",
                f.source.path().join("fastas").to_str().unwrap(),
                "--expected-anim-sha256",
                &common::package::sha256_hex(
                    &fs::read(f.source.path().join("fastas/anim.json")).unwrap(),
                ),
                "--anim-output",
                f.source.path().join("explicit-evidence").to_str().unwrap(),
            ])
            .arg(&f.query)
            .output()
            .unwrap(),
    );
    for key in [
        "species_id",
        "subspecies_id",
        "rescue_status",
        "call_basis",
        "ranked_reference_evidence",
        "top_references",
        "panel_summary",
        "counts",
        "input",
        "legacy_v1_result",
    ] {
        assert_eq!(managed[key], explicit[key], "scientific field {key}");
    }
    let again = value(
        f.command(&store)
            .args(["--database", "fixture-1"])
            .arg(&f.query)
            .output()
            .unwrap(),
    );
    let next = again["data_provenance"]["anim_evidence_directory"]
        .as_str()
        .unwrap();
    assert_ne!(next, evidence);
    assert!(Path::new(evidence).join("query-000001").is_dir());
}
#[test]
fn conflicts_and_unpinned_defaults_have_no_side_effects() {
    let mut f = Fixture::new();
    let store = f.source.path().join("store");
    let out = f
        .command(&store)
        .args([
            "--database",
            "fixture-1",
            "--reference-package",
            "refs",
            "--expected-sha256",
            &f.reference,
        ])
        .arg(&f.query)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(!store.exists());
    let out = f.command(&store).arg(&f.query).output().unwrap();
    assert!(!out.status.success());
    assert!(!store.exists());
    f.catalogue["releases"][0]["default"] = true.into();
    f.save();
    let out = f
        .command(&store)
        .args(["database", "list"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(!store.exists());
}
#[test]
fn skani_only_install_allows_missing_genomes_but_default_workflow_refuses_it() {
    let f = Fixture::new();
    let store = f.source.path().join("store");
    install(&f, &store, &["--no-anim"]);
    value(
        f.command(&store)
            .args(["--database", "fixture-1", "--no-anim"])
            .arg(&f.query)
            .output()
            .unwrap(),
    );
    let out = f
        .command(&store)
        .args(["--database", "fixture-1"])
        .arg(&f.query)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(!f.source.path().join("state").exists());
}
#[test]
fn version_and_list_do_not_create_user_state() {
    let f = Fixture::new();
    let store = f.source.path().join("store");
    value(
        f.command(&store)
            .args(["database", "list"])
            .output()
            .unwrap(),
    );
    assert!(!store.exists());
    let output = assert_cmd::cargo::cargo_bin_cmd!("ptero")
        .arg("--version")
        .env_remove("HOME")
        .env_remove("XDG_DATA_HOME")
        .env_remove("XDG_STATE_HOME")
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn catalogue_rejects_untrusted_or_inconsistent_metadata_before_store_creation() {
    let mut f = Fixture::new();
    let original = f.catalogue.clone();
    let store = f.source.path().join("store");
    for variant in 0..10 {
        f.catalogue = original.clone();
        let r = &mut f.catalogue["releases"][0];
        match variant {
            0 => r["default"] = true.into(),
            1 => r["components"][0]["members"][0]["path"] = "../outside".into(),
            2 => r["components"][0]["extracted"]["bytes"] = 0.into(),
            3 => r["components"][0]["identity"]["sha256"] = "0".repeat(64).into(),
            4 => r["components"][1]["binds"]["reference"] = "0".repeat(64).into(),
            5 => {
                r["components"][0]["archive"]["urls"] =
                    serde_json::json!(["https://example.org/candidate"])
            }
            6 => {
                r["status"] = "published".into();
                r["components"][0]["archive"]["urls"] =
                    serde_json::json!(["http://example.org/insecure"]);
            }
            7 => r["version"] = "../outside".into(),
            8 => {
                let duplicate = r["components"][0].clone();
                r["components"].as_array_mut().unwrap().push(duplicate);
            }
            9 => r["components"][0]["archive"]
                .as_object_mut()
                .unwrap()
                .remove("tar_bytes")
                .map(|_| ())
                .unwrap(),
            _ => unreachable!(),
        }
        f.save();
        let out = f
            .command(&store)
            .args(["database", "list"])
            .output()
            .unwrap();
        assert!(!out.status.success(), "invalid variant {variant}");
        assert!(!store.exists());
    }
    f.catalogue = original;
    f.save();
    for extra in [
        vec!["--no-anim", "database", "list"],
        vec!["database", "install"],
        vec!["database", "verify"],
        vec!["--threads", "1", "database", "list"],
    ] {
        let out = f.command(&store).args(extra).output().unwrap();
        assert!(!out.status.success());
        assert!(!store.exists());
    }
}

#[test]
fn mismatched_anim_payload_preserves_reference_and_can_be_retried_with_correct_archive() {
    let mut f = Fixture::new();
    let store = f.source.path().join("store");
    let original = f.catalogue.clone();
    let source = f.archive_dir.join("primary-fasta.tar.gz");
    let bytes = fs::read(&source).unwrap();
    let mut files = common::database::tree(&f.source.path().join("fastas"));
    let mut manifest: Value = serde_json::from_slice(&files["anim.json"]).unwrap();
    manifest["reference_package_sha256"] = "0".repeat(64).into();
    files.insert(
        "anim.json".into(),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    );
    let (component, bad) = common::database::archive(
        "primary-fasta",
        files,
        "anim.json",
        "default_workflow",
        serde_json::json!({"reference":f.reference}),
    );
    f.catalogue["releases"][0]["components"][1] = component;
    f.save();
    fs::write(&source, bad).unwrap();
    let out = f
        .command(&store)
        .args([
            "database",
            "install",
            "fixture-1",
            "--from-directory",
            f.archive_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(store
        .join("synthetic/fixture-1/reference/portable.json")
        .exists());
    assert!(!store.join("synthetic/fixture-1/primary-fasta").exists());
    f.catalogue = original;
    f.save();
    fs::write(source, bytes).unwrap();
    let success = install(&f, &store, &[]);
    assert_eq!(success["components"][0]["status"], "verified_reuse");
    assert_eq!(success["components"][1]["status"], "installed");
}

#[test]
fn published_default_and_withdrawn_explicit_selection_never_fall_back() {
    let mut f = Fixture::new();
    let store = f.source.path().join("store");
    f.catalogue["releases"][0]["status"] = "published".into();
    f.catalogue["releases"][0]["default"] = true.into();
    f.save();
    install(&f, &store, &["--no-anim"]);
    let report = value(
        f.command(&store)
            .arg("--no-anim")
            .arg(&f.query)
            .output()
            .unwrap(),
    );
    assert_eq!(
        report["data_provenance"]["reference_trust"],
        "operator_pinned_catalogue"
    );
    f.catalogue["releases"][0]["status"] = "withdrawn".into();
    f.catalogue["releases"][0]["default"] = false.into();
    f.save();
    let out = f
        .command(&store)
        .args(["--database", "fixture-1", "--no-anim"])
        .arg(&f.query)
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&out.stderr).contains("withdrawn"));
    value(out);
    let out = f
        .command(&store)
        .arg("--no-anim")
        .arg(&f.query)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let out = f
        .command(&store)
        .args(["--database", "latest", "--no-anim"])
        .arg(&f.query)
        .output()
        .unwrap();
    assert!(!out.status.success());
}

#[test]
fn managed_custom_scheme_and_explicit_typing_override_preserve_the_taxonomy() {
    let mut f = Fixture::new();
    let custom = common::mlst::custom_scheme();
    let (component, bytes) = common::database::archive(
        "mlst-custom",
        custom.files,
        "manifest.json",
        "optional",
        serde_json::json!({}),
    );
    f.catalogue["releases"][0]["components"][2] = component;
    let schemas = f.catalogue["releases"][0]["schemas"]
        .as_object_mut()
        .unwrap();
    schemas.remove("mlst-historical");
    schemas.insert("mlst-custom".into(), 2.into());
    f.save();
    fs::write(f.archive_dir.join("mlst-custom.tar.gz"), bytes).unwrap();
    let store = f.source.path().join("store");
    install(&f, &store, &["--mlst", "--no-anim"]);
    let plain = value(
        f.command(&store)
            .args(["-D", "fixture-1", "-n"])
            .arg(&f.query)
            .output()
            .unwrap(),
    );
    let typed = value(
        f.command(&store)
            .args(["-D", "fixture-1", "-n", "-l"])
            .arg(&f.query)
            .output()
            .unwrap(),
    );
    assert_eq!(typed["mlst"]["database"]["package_scope"], "custom");
    assert_eq!(typed["mlst"]["database"]["submission_cutoff"], Value::Null);
    assert_eq!(plain["assignment"], typed["assignment"]);
    let historical = common::mlst::SyntheticScheme::new();
    let explicit = f.source.path().join("explicit-historical");
    historical.write_to(&explicit);
    let typed = value(
        f.command(&store)
            .args([
                "-D",
                "fixture-1",
                "-n",
                "-l",
                "-m",
                explicit.to_str().unwrap(),
                "-M",
                &historical.manifest_sha256(),
            ])
            .arg(&f.query)
            .output()
            .unwrap(),
    );
    assert_eq!(typed["mlst"]["database"]["submission_cutoff"], "2024-12-31");
    assert_eq!(typed["data_provenance"]["mlst_trust"], "operator_pin");
    assert_eq!(plain["assignment"], typed["assignment"]);
}
