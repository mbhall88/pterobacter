//! Synthetic metrics exercised through the real command against an authenticated frozen package.
//! No sequence search is performed. The private package is supplied only for its pinned metadata.
#![cfg(feature = "dev")]

mod common;

use assert_cmd::cargo::cargo_bin_cmd;
use common::package::sha256_hex;
use serde_json::{json, Value};
use std::{fs, path::Path, process::Output};
use terobacter::genomovar::{self, Context, Definition, GenomovarParent, MemberRow};
use terobacter::reference::{verify_package, RegistryPins, VerifiedPackage};

struct PackageDirectory(std::path::PathBuf);
impl terobacter::reference::PackageSource for PackageDirectory {
    fn open(&self, path: &str) -> std::io::Result<Option<Box<dyn std::io::Read + '_>>> {
        Ok(Some(Box::new(fs::File::open(self.0.join(path))?)))
    }
    fn inventory(&self) -> std::io::Result<std::collections::BTreeSet<String>> {
        fn walk(
            root: &Path,
            dir: &Path,
            files: &mut std::collections::BTreeSet<String>,
        ) -> std::io::Result<()> {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let kind = entry.file_type()?;
                if kind.is_symlink() {
                    return Err(std::io::Error::other(
                        "test package must not contain symlinks",
                    ));
                }
                if kind.is_dir() {
                    walk(root, &entry.path(), files)?;
                } else {
                    files.insert(
                        entry
                            .path()
                            .strip_prefix(root)
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .into(),
                    );
                }
            }
            Ok(())
        }
        let mut files = std::collections::BTreeSet::new();
        walk(&self.0, &self.0, &mut files)?;
        Ok(files)
    }
}

fn primary_evidence(package: &VerifiedPackage, pin: &str) -> Value {
    let mut references: Vec<Value> = package.component("primary_whole").unwrap().references.iter().map(|r| {
        let species = package.concept(&r.species_id).unwrap();
        let subspecies = r.subspecies_id.as_deref().and_then(|id| package.concept(id));
        let ani = if r.subspecies_id.as_deref() == Some(genomovar::PARENT_SUBSPECIES_ID) { 100 } else { 90 };
        json!({"reference_id": r.reference_id, "species_id": species.id,
            "species_name": species.display_name,
            "subspecies_id": subspecies.map_or("", |c| c.id.as_str()),
            "subspecies_name": subspecies.map_or("", |c| c.display_name.as_str()),
            "lineage_id": "", "reference_role": if r.type_anchor { "type_anchor" } else { "diversity" },
            "name_status": subspecies.unwrap_or(species).nomenclature_status,
            "ani_percent": ani, "aligned_fraction_query_percent": 95,
            "aligned_fraction_reference_percent": 95, "minimum_directional_af_percent": 95})
    }).collect();
    references.sort_by_key(|r| {
        (
            std::cmp::Reverse(r["ani_percent"].as_u64().unwrap()),
            r["reference_id"].as_str().unwrap().to_owned(),
        )
    });
    let nearest: Vec<_> = references
        .iter()
        .take_while(|r| r["ani_percent"] == 100)
        .map(|r| r["reference_id"].clone())
        .collect();
    assert!(
        nearest.len() >= 2,
        "retained-parent scenario needs two Group A primary references"
    );
    let mut evidence = common::evidence_json(&[]);
    evidence["provenance"]["reference_package_sha256"] = pin.into();
    evidence["reference_count"] = references.len().into();
    evidence["raw_hit_count"] = references.len().into();
    evidence["unique_hit_count"] = references.len().into();
    evidence["nearest_reference_ids"] = nearest.into();
    evidence["ranked_reference_evidence"] = references.into();
    evidence
}

fn report(
    package: &VerifiedPackage,
    definition: &Definition,
    evidence: &Value,
    target: &str,
    rival: &str,
    v2: bool,
) -> Value {
    let saved = terobacter::evidence::SavedEvidence::from_json(&evidence.to_string()).unwrap();
    let mut report = if v2 {
        genomovar::diagnostic_parent(
            saved,
            package,
            &[],
            &genomovar::DiagnosticParentPolicy::V2(terobacter::rescue::Thresholds::standard()),
            &terobacter::rescue::AnimRecords::default(),
            None,
        )
        .unwrap()
    } else {
        terobacter::decision::decide_bound(&saved, package, &terobacter::decision::Policy::v1())
            .unwrap()
            .to_value()
            .clone()
    };
    report["evidence"] = evidence.clone();
    report["input"]["sequence_scope"] = genomovar::QUERY_SCOPE.into();
    report["input"]["verification"] = "query_bytes_hashed; reference_package_verified".into();
    report["execution"] = json!({"status": "complete", "stage": "genomovar"});
    let parent = GenomovarParent::from_report(&report);
    assert!(parent.eligibility_reasons().is_empty(), "{report}");
    let component = package.component(genomovar::COMPONENT_ID).unwrap();
    let rows: Vec<Value> = definition
        .groups()
        .iter()
        .flat_map(|(group, members)| {
            members.iter().enumerate().map(move |(index, id)| {
                json!({"reference_id": id,
            "ani_percent": if group == target || (group == rival && index >= 2) { 100 } else { 99 },
            "aligned_fraction_query_percent": 95, "aligned_fraction_reference_percent": 95})
            })
        })
        .collect();
    let typed: Vec<MemberRow> = serde_json::from_value(json!(rows)).unwrap();
    let context = Context {
        parent,
        input_kind: "assembly".into(),
        query_scope: genomovar::QUERY_SCOPE.into(),
        reference_scope: component.sequence_scope.clone(),
        engine: component.engine.clone(),
        engine_version: component.engine_version.clone(),
        parameters: genomovar::SEARCH_PARAMETERS.into(),
        search_status: "completed".into(),
        adapter_verified: true,
        query_sha256: report["input"]["query_sha256"].as_str().unwrap().into(),
        package_sha256: report["input"]["reference_package_sha256"]
            .as_str()
            .unwrap()
            .into(),
        diagnostic_exclusion: false,
    };
    let mut section = genomovar::assess(definition, &typed, &context)
        .unwrap()
        .to_value();
    assert_eq!(section["genomovar_id"], target);
    section["search"]["component_id"] = genomovar::COMPONENT_ID.into();
    section["search"]["reference_count"] = component.references.len().into();
    section["search"]["returned_count"] = rows.len().into();
    section["search"]["member_evidence"] = rows.into();
    section["search"]["not_returned_reference_ids"] = json!([]);
    if v2 {
        report["genomovar_id"] = target.into();
    } else {
        report["assignment"]["genomovar_id"] = target.into();
    }
    report["genomovar"] = section;
    report
}

fn run(report: &Value, temp: &Path, package: &str, pin: &str, extra: &[(&str, &str)]) -> Output {
    let path = temp.join("source-report.json");
    let bytes = serde_json::to_vec_pretty(report).unwrap();
    fs::write(&path, &bytes).unwrap();
    let hash = sha256_hex(&bytes);
    let mut command = cargo_bin_cmd!("pterobacter-dev");
    command.args([
        "genomovar-diagnostic",
        "--report",
        path.to_str().unwrap(),
        "--expected-report-sha256",
        &hash,
        "--reference-package",
        package,
        "--expected-sha256",
        pin,
    ]);
    for (flag, value) in extra {
        command.args([*flag, *value]);
    }
    let output = command.output().unwrap();
    assert_eq!(
        sha256_hex(&fs::read(path).unwrap()),
        hash,
        "source report changed"
    );
    output
}

fn successful(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["genomovar_id"], Value::Null);
    assert_eq!(result["diagnostic"]["genomovar_id"], Value::Null);
    result
}

fn refused(output: Output, reason: &str) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(reason),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Set TEROBACTER_FROZEN_PACKAGE and TEROBACTER_FROZEN_PACKAGE_SHA256, then run this test with
/// --ignored. All metrics are synthetic; the package's databases are never searched.
#[test]
#[ignore = "requires private frozen package metadata; generates only synthetic evidence"]
fn command_preserves_membership_and_binds_masked_or_withheld_parents() {
    let directory = std::env::var("TEROBACTER_FROZEN_PACKAGE").unwrap();
    let pin = std::env::var("TEROBACTER_FROZEN_PACKAGE_SHA256").unwrap();
    let package = verify_package(
        &PackageDirectory(directory.clone().into()),
        &pin,
        &RegistryPins::default(),
    )
    .unwrap();
    let definition = Definition::from_package(&package).unwrap();
    let mut multimember = definition
        .groups()
        .iter()
        .filter(|(_, members)| members.len() >= 2);
    let (target, own) = multimember.next().unwrap();
    let (rival, rival_members) = multimember.next().unwrap();
    let primary = package.component("primary_whole").unwrap();
    let absent_primary_member = own
        .iter()
        .find(|id| {
            let accession = &package.reference(id).unwrap().accession;
            !primary
                .references
                .iter()
                .any(|reference| &reference.accession == accession)
        })
        .expect("empty-mask regression needs a target member absent from the primary panel");
    let evidence = primary_evidence(&package, &pin);
    let parent_members: Vec<String> = package
        .component("primary_whole")
        .unwrap()
        .references
        .iter()
        .filter(|r| r.subspecies_id.as_deref() == Some(genomovar::PARENT_SUBSPECIES_ID))
        .map(|r| r.reference_id.clone())
        .collect();
    let temp = tempfile::tempdir().unwrap();
    let self_mask = temp.path().join("self-mask.txt");
    fs::write(&self_mask, format!("{}\n", parent_members[0])).unwrap();
    let self_pin = sha256_hex(&fs::read(&self_mask).unwrap());
    let veto_mask = temp.path().join("parent-veto-mask.txt");
    fs::write(&veto_mask, parent_members.join("\n")).unwrap();
    let veto_pin = sha256_hex(&fs::read(&veto_mask).unwrap());
    let empty_mask = temp.path().join("empty-primary-mask.txt");
    fs::write(&empty_mask, "").unwrap();
    let empty_pin = sha256_hex(b"");
    let invalid_mask = temp.path().join("unknown-primary-mask.txt");
    fs::write(&invalid_mask, "unknown-primary-reference\n").unwrap();
    let invalid_pin = sha256_hex(&fs::read(&invalid_mask).unwrap());
    let missing_mask = temp.path().join("missing-primary-mask.txt");
    let warning_artifact = temp.path().join("warning.txt");
    fs::write(&warning_artifact, "synthetic concern").unwrap();

    for v2 in [false, true] {
        let baseline = report(&package, &definition, &evidence, target, rival, v2);
        let ordinary = successful(run(&baseline, temp.path(), &directory, &pin, &[]));
        assert_eq!(
            ordinary["diagnostic_would_assign_genomovar_id"],
            target.as_str()
        );

        let missing_own = successful(run(
            &baseline,
            temp.path(),
            &directory,
            &pin,
            &[("--exclude-evidence-row", &own[0])],
        ));
        assert_eq!(
            missing_own["diagnostic_would_assign_genomovar_id"],
            Value::Null
        );
        assert_eq!(
            missing_own["diagnostic"]["incomplete_groups"],
            json!([target])
        );
        assert_eq!(
            missing_own["diagnostic"]["definition"],
            ordinary["diagnostic"]["definition"]
        );
        let one_contradiction = successful(run(
            &baseline,
            temp.path(),
            &directory,
            &pin,
            &[("--exclude-evidence-row", &rival_members[0])],
        ));
        assert_eq!(
            one_contradiction["diagnostic_would_assign_genomovar_id"],
            target.as_str()
        );
        let no_contradiction = successful(run(
            &baseline,
            temp.path(),
            &directory,
            &pin,
            &[
                ("--exclude-evidence-row", &rival_members[0]),
                ("--exclude-evidence-row", &rival_members[1]),
            ],
        ));
        assert_eq!(
            no_contradiction["diagnostic_would_assign_genomovar_id"],
            Value::Null
        );
        assert_eq!(
            no_contradiction["diagnostic"]["incomplete_groups"],
            json!([rival])
        );
        assert_eq!(
            no_contradiction["diagnostic"]["definition"],
            ordinary["diagnostic"]["definition"]
        );

        let matching = [
            ("--setting", "self_alias"),
            ("--exclude-references", self_mask.to_str().unwrap()),
            ("--exclude-references-sha256", self_pin.as_str()),
        ];
        let mut retained_args = matching.to_vec();
        retained_args.push(("--mask-member", &own[0]));
        let retained = successful(run(
            &baseline,
            temp.path(),
            &directory,
            &pin,
            &retained_args,
        ));
        assert_eq!(
            retained["diagnostic_would_assign_genomovar_id"],
            target.as_str()
        );
        assert_eq!(
            retained["parent_recomputation"]["excluded_reference_ids"],
            json!([parent_members[0]])
        );
        assert_eq!(retained["parent_recomputation"]["setting"], "self_alias");

        let lost = successful(run(
            &baseline,
            temp.path(),
            &directory,
            &pin,
            &[
                ("--setting", "clone_block"),
                ("--exclude-references", veto_mask.to_str().unwrap()),
                ("--exclude-references-sha256", &veto_pin),
            ],
        ));
        assert_eq!(lost["diagnostic_would_assign_genomovar_id"], Value::Null);
        assert_eq!(lost["diagnostic_outcome"], "gv_withheld_parent");
        assert!(lost["diagnostic"]["reasons"]
            .as_array()
            .unwrap()
            .contains(&json!("absent_eligible_parent")));

        let warnings = temp.path().join("warnings.json");
        let envelope = json!({"schema_version": "1.0-shadow-problem-warnings",
            "query_sha256": baseline["input"]["query_sha256"], "reference_package_sha256": pin,
            "evidence_sha256": baseline["input"]["evidence_sha256"],
            "warnings": [{"warning_id": "synthetic-concern", "reason": "Synthetic concern", "source": "synthetic-test",
                "source_version": "1", "artifacts": [{"path": warning_artifact, "sha256": sha256_hex(b"synthetic concern")}]}]});
        fs::write(&warnings, envelope.to_string()).unwrap();
        let warning_pin = sha256_hex(&fs::read(&warnings).unwrap());
        let warned = successful(run(
            &baseline,
            temp.path(),
            &directory,
            &pin,
            &[
                ("--problem-warnings", warnings.to_str().unwrap()),
                ("--problem-warnings-sha256", &warning_pin),
            ],
        ));
        assert_eq!(warned["diagnostic_outcome"], "gv_withheld_parent");
        assert_eq!(warned["diagnostic_would_assign_genomovar_id"], Value::Null);
        assert!(warned["diagnostic"]["parent"]["blocking_reasons"]
            .as_array()
            .unwrap()
            .contains(&json!("external_problem_warning")));

        for setting in ["self_alias", "clone_block"] {
            let empty_args = [
                ("--setting", setting),
                ("--exclude-references", empty_mask.to_str().unwrap()),
                ("--exclude-references-sha256", empty_pin.as_str()),
                ("--mask-member", absent_primary_member.as_str()),
            ];
            let retained = successful(run(&baseline, temp.path(), &directory, &pin, &empty_args));
            assert_eq!(
                retained["diagnostic_would_assign_genomovar_id"],
                target.as_str()
            );
            assert_eq!(retained["parent_recomputation"]["setting"], setting);
            assert_eq!(
                retained["parent_recomputation"]["primary_mask_sha256"],
                empty_pin
            );
            assert_eq!(
                retained["parent_recomputation"]["excluded_reference_ids"],
                json!([])
            );
            let mut veto_args = empty_args.to_vec();
            veto_args.extend([
                ("--problem-warnings", warnings.to_str().unwrap()),
                ("--problem-warnings-sha256", warning_pin.as_str()),
            ]);
            let vetoed = successful(run(&baseline, temp.path(), &directory, &pin, &veto_args));
            assert_eq!(vetoed["diagnostic_outcome"], "gv_withheld_parent");
            assert_eq!(vetoed["diagnostic_would_assign_genomovar_id"], Value::Null);
            assert_eq!(vetoed["parent_recomputation"]["setting"], setting);
            assert_eq!(
                vetoed["parent_recomputation"]["excluded_reference_ids"],
                json!([])
            );
            assert!(vetoed["diagnostic"]["parent"]["blocking_reasons"]
                .as_array()
                .unwrap()
                .contains(&json!("external_problem_warning")));
        }
        // An empty, authenticated mask differs from missing, unpinned, mismatched or invalid input.
        for (path, expected, reason) in [
            (empty_mask.as_path(), None, "--exclude-references-sha256"),
            (
                empty_mask.as_path(),
                Some(self_pin.as_str()),
                "SHA-256 mismatch",
            ),
            (
                missing_mask.as_path(),
                Some(empty_pin.as_str()),
                "missing-primary-mask.txt",
            ),
            (
                invalid_mask.as_path(),
                Some(invalid_pin.as_str()),
                "unknown",
            ),
        ] {
            let mut args = vec![
                ("--setting", "self_alias"),
                ("--exclude-references", path.to_str().unwrap()),
            ];
            if let Some(hash) = expected {
                args.push(("--exclude-references-sha256", hash));
            }
            refused(run(&baseline, temp.path(), &directory, &pin, &args), reason);
        }
        for field in ["query_sha256", "reference_package_sha256"] {
            let mut mismatch = baseline.clone();
            mismatch["evidence"]["provenance"][field] = "f".repeat(64).into();
            refused(
                run(&mismatch, temp.path(), &directory, &pin, &matching),
                "primary evidence does not match",
            );
        }
        let mut mismatch = baseline.clone();
        mismatch["evidence"]["ranked_reference_evidence"][0]["aligned_fraction_query_percent"] =
            94.into();
        mismatch["evidence"]["ranked_reference_evidence"][0]["minimum_directional_af_percent"] =
            94.into();
        refused(
            run(&mismatch, temp.path(), &directory, &pin, &matching),
            "primary evidence does not match",
        );
        refused(
            run(
                &baseline,
                temp.path(),
                &directory,
                &pin,
                &[("--mask-member", &own[0])],
            ),
            "member masks require",
        );
        refused(
            run(
                &baseline,
                temp.path(),
                &directory,
                &pin,
                &[("--setting", "self_alias")],
            ),
            "member masks require",
        );
        refused(
            run(
                &baseline,
                temp.path(),
                &directory,
                &pin,
                &[("--exclude-evidence-row", "unknown-member")],
            ),
            "unknown, absent or duplicate",
        );
        refused(
            run(
                &baseline,
                temp.path(),
                &directory,
                &pin,
                &[
                    ("--exclude-evidence-row", &own[0]),
                    ("--exclude-evidence-row", &own[0]),
                ],
            ),
            "unknown, absent or duplicate",
        );
    }
}
