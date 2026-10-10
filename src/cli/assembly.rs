//! Whole-assembly I/O and orchestration, separate from pure decision rules.
use crate::evidence::sha256_hex;
use serde_json::{json, Value};
use std::path::Path;

use crate::decision::{decide_bound, Policy};
use crate::evidence::{SavedEvidence, EVIDENCE_SCHEMA};
use crate::reference::{verify_package, RegistryPins, VerifiedPackage};
use crate::skani::{SearchReport, SearchSettings, SeedingMode, SkaniDatabase};
use std::path::PathBuf;

/// Authenticated primary panel and its embedded measurement engine. Open once
/// for a sequential batch; finish rechecks package bytes before reports leave.
pub struct AssemblyWorkflow {
    directory: PathBuf,
    expected_sha256: String,
    package: VerifiedPackage,
    database: SkaniDatabase,
}
impl AssemblyWorkflow {
    pub fn open(directory: &Path, expected_sha256: &str) -> Result<Self, String> {
        let package = verify_package(
            &crate::cli::Directory(directory.to_owned()),
            expected_sha256,
            &RegistryPins::default(),
        )
        .map_err(|e| e.to_string())?;
        let component = package
            .component("primary_whole")
            .ok_or("package has no primary component")?;
        let parent = Path::new(&component.databases["index.db"].path)
            .parent()
            .ok_or("index has no directory")?;
        let database = SkaniDatabase::open(&directory.join(parent)).map_err(|e| e.to_string())?;
        Ok(Self {
            directory: directory.to_owned(),
            expected_sha256: expected_sha256.into(),
            package,
            database,
        })
    }

    /// Classify validated whole-input bytes with v1, retaining each completed
    /// stage in the partial report if a later stage fails.
    pub fn analyse(
        &self,
        query_label: &str,
        bytes: &[u8],
        threads: usize,
        report: &mut Value,
    ) -> Result<(), String> {
        let saved = self.measure(query_label, bytes, threads, report)?;
        let evidence_value = report["evidence"].clone();
        report["execution"]["stage"] = "decision".into();
        let decision =
            decide_bound(&saved, &self.package, &Policy::v1()).map_err(|e| e.to_string())?;
        let mut complete = decision.to_value().clone();
        for (key, value) in report["input"]
            .as_object()
            .ok_or("report has no input metadata")?
        {
            complete["input"][key] = value.clone();
        }
        complete["input"]["query_sha256"] = sha256_hex(bytes).into();
        complete["input"]["verification"] = "query_bytes_hashed; reference_package_verified".into();
        complete["input"]["sequence_scope"] = "whole_assembly_as_supplied".into();
        complete["measurement"] = report["measurement"].take();
        complete["evidence"] = evidence_value;
        complete["execution"] = json!({"status": "complete", "stage": "decision"});
        *report = complete;
        Ok(())
    }

    fn measure(
        &self,
        query_label: &str,
        bytes: &[u8],
        threads: usize,
        report: &mut Value,
    ) -> Result<SavedEvidence, String> {
        report["reference_package"] = self.package.to_value();
        report["execution"]["stage"] = "measurement".into();
        let measurement = self
            .database
            .search_bytes(
                query_label,
                bytes,
                SearchSettings::Primary,
                threads,
                SeedingMode::Auto,
            )
            .map_err(|e| e.to_string())?;
        report["measurement"] = serde_json::to_value(&measurement).map_err(|e| e.to_string())?;
        report["execution"]["stage"] = "evidence".into();
        let evidence_value = self.evidence(&measurement, &sha256_hex(bytes))?;
        report["evidence"] = evidence_value.clone();
        SavedEvidence::from_json(&evidence_value.to_string()).map_err(|e| e.to_string())
    }

    /// Run the accepted v2 planner after whole-assembly skani measurement.
    pub fn analyse_v2(
        &self,
        query_label: &str,
        bytes: &[u8],
        threads: usize,
        report: &mut Value,
        context: &super::v2::V2Context<'_>,
    ) -> Result<(), String> {
        let saved = self.measure(query_label, bytes, threads, report)?;
        let input = crate::rescue::RescueInput::new(
            saved.clone(),
            self.panel(&saved)?,
            &[],
            crate::rescue::Thresholds::standard(),
        )
        .map_err(|e| e.to_string())?;
        report["execution"]["stage"] = "anim".into();
        let run = super::v2::execute(&input, &self.package, bytes, context);
        let mut complete = input
            .decide(&run.bound)
            .map_err(|e| e.to_string())?
            .to_value()
            .clone();
        complete["input"] = merge_input(complete["input"].clone(), report["input"].clone());
        complete["input"]["verification"] = "query_bytes_hashed; reference_package_verified".into();
        complete["reference_package"] = self.package.to_value();
        complete["measurement"] = report["measurement"].take();
        complete["evidence"] = report["evidence"].take();
        complete["anim_plan"] = run.plan;
        complete["anim_execution"] = run.execution;
        complete["input"]["anim"]["source"] = "generated_native_pairs".into();
        // A failed snapshot or rejected record may have no bindable pair. Count the actual
        // native attempt independently of the pure evidence parser.
        let pairs = complete["anim_execution"]["pairs"]
            .as_array()
            .expect("execution pairs");
        let attempted = pairs.len();
        let completed = pairs.iter().filter(|p| p["status"] == "complete").count();
        complete["counts"]["attempted"] = attempted.into();
        complete["counts"]["completed"] = completed.into();
        complete["counts"]["failed"] = (attempted - completed).into();
        complete["counts"]["not_run"] = (complete["counts"]["required"]
            .as_u64()
            .expect("required count")
            .saturating_sub(attempted as u64))
        .into();
        complete["genomovar_id"] = Value::Null;
        complete["execution"] = json!({"status": "complete", "stage": "decision"});
        *report = complete;
        if let Some(error) = run.error {
            report["execution"] = json!({"status": "incomplete", "stage": "anim"});
            super::withhold(report);
            return Err(error);
        }
        if report["evaluation_status"] == "incomplete"
            && report["anim_execution"]["stop_reason"] != "decisive_veto"
        {
            report["execution"] = json!({"status": "incomplete", "stage": "decision"});
            super::withhold(report);
            return Err("required v2 evidence is incomplete".into());
        }
        Ok(())
    }

    /// Recheck the independently manifested FASTAs before batch publication.
    pub fn finish_anim(&self, settings: &super::v2::V2Settings) -> Result<(), String> {
        let directory = settings
            .anim_reference_package
            .as_ref()
            .ok_or("missing ANIm directory")?;
        let expected = settings
            .expected_anim_sha256
            .as_deref()
            .ok_or("missing ANIm digest")?;
        super::v2::verify_component(directory, expected, &self.package)?;
        Ok(())
    }

    fn reference_metadata(&self, reference: &crate::reference::PackageReference) -> Value {
        let species = self
            .package
            .concept(&reference.species_id)
            .expect("verified species");
        let subspecies = reference
            .subspecies_id
            .as_deref()
            .and_then(|id| self.package.concept(id));
        json!({"reference_id": reference.reference_id,
            "species_id": species.id, "species_name": species.display_name,
            "subspecies_id": subspecies.map_or("", |c| c.id.as_str()),
            "subspecies_name": subspecies.map_or("", |c| c.display_name.as_str()),
            "lineage_id": "", "reference_role": if reference.type_anchor { "type_anchor" } else { "diversity" },
            "name_status": subspecies.unwrap_or(species).nomenclature_status})
    }

    fn panel(&self, saved: &SavedEvidence) -> Result<crate::panel::Panel, String> {
        let references: Vec<Value> = self
            .package
            .component("primary_whole")
            .expect("primary component")
            .references
            .iter()
            .map(|r| {
                let mut row = self.reference_metadata(r);
                row["source_sha256"] = r.source.sha256.clone().into();
                row
            })
            .collect();
        let value = json!({"schema_version": "1.0-shadow-primary-inventory", "package_sha256": self.expected_sha256,
            "component_id": "primary_whole", "representation": {"source": "authenticated_package"}, "references": references});
        crate::panel::Panel::from_json(&value.to_string(), saved).map_err(|e| e.to_string())
    }

    /// Reject a package changed during a run, including an unreturned reference.
    pub fn finish(&self) -> Result<(), String> {
        verify_package(
            &crate::cli::Directory(self.directory.clone()),
            &self.expected_sha256,
            &RegistryPins::default(),
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn evidence(&self, measurement: &SearchReport, query_sha256: &str) -> Result<Value, String> {
        let component = self
            .package
            .component("primary_whole")
            .expect("checked primary component");
        let by_label: std::collections::BTreeMap<_, _> = component
            .references
            .iter()
            .map(|r| (r.search_label.as_str(), r))
            .collect();
        let mut seen = std::collections::BTreeSet::new();
        let mut rows = Vec::new();
        for row in &measurement.rows {
            let reference = by_label
                .get(row.reference_file.as_str())
                .ok_or("unbound skani reference")?;
            if !seen.insert(&reference.reference_id) {
                return Err("duplicate skani reference".into());
            }
            let mut value = self.reference_metadata(reference);
            value["ani_percent"] = json!(row.ani);
            value["aligned_fraction_query_percent"] = json!(row.af_query);
            value["aligned_fraction_reference_percent"] = json!(row.af_reference);
            value["minimum_directional_af_percent"] = json!(row.af_query.min(row.af_reference));
            rows.push(value);
        }
        for label in &measurement.not_returned_reference_files {
            let reference = by_label
                .get(label.as_str())
                .ok_or("unbound unreturned skani reference")?;
            if !seen.insert(&reference.reference_id) {
                return Err("duplicate unreturned reference".into());
            }
        }
        if seen.len() != by_label.len() {
            return Err("skani did not assess the complete primary panel".into());
        }
        rows.sort_by_cached_key(|row| {
            let ani: crate::evidence::Percent =
                serde_json::from_value(row["ani_percent"].clone()).expect("emitted percentage");
            (
                std::cmp::Reverse(ani),
                row["reference_id"]
                    .as_str()
                    .expect("reference ID")
                    .to_owned(),
            )
        });
        let nearest: Vec<_> = rows
            .iter()
            .take_while(|row| row["ani_percent"] == rows[0]["ani_percent"])
            .map(|row| row["reference_id"].clone())
            .collect();
        Ok(
            json!({"schema_version": EVIDENCE_SCHEMA, "record_type": "development_evidence_only",
            "input_kind": "assembly", "units": {"ani": "percent", "aligned_fraction": "percent"},
            "assignment": {"status": "not_evaluated", "reason": "native_decision_pending",
                "species_id": null, "species_name": null, "subspecies_id": null,
                "subspecies_name": null, "lineage_id": null, "genomovar_id": null},
            "provenance": {"input_kind": "assembly", "engine_version": measurement.engine_version,
                "reference_component_id": "primary_whole", "reference_package_sha256": self.expected_sha256,
                "query_sha256": query_sha256, "sequence_scope": "whole_assembly_as_supplied", "search_exit_code": 0},
            "evidence_status": if rows.is_empty() { "no_estimate_returned" } else { "estimates_returned" },
            "reference_count": by_label.len(), "raw_hit_count": rows.len(), "unique_hit_count": rows.len(),
            "not_returned_reference_count": measurement.not_returned_reference_files.len(),
            "duplicate_rows_collapsed": 0, "nearest_reference_ids": nearest, "ranked_reference_evidence": rows}),
        )
    }
}

fn merge_input(mut target: Value, source: Value) -> Value {
    for (key, value) in source.as_object().expect("input metadata") {
        target[key] = value.clone();
    }
    target
}
