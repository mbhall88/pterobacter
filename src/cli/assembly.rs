//! Whole-assembly I/O and orchestration, separate from pure decision rules.
use crate::evidence::sha256_hex;
use serde_json::{json, Value};
use std::path::Path;

use crate::decision::{decide_bound, Policy};
use crate::evidence::{SavedEvidence, EVIDENCE_SCHEMA};
use crate::reference::{verify_package, RegistryPins, VerifiedPackage};
use crate::skani::{SearchReport, SearchSettings, SeedingMode, SkaniDatabase};
use std::cell::OnceCell;
use std::path::PathBuf;

use crate::genomovar::{self, Context, Definition, GenomovarParent, MemberRow};
use std::collections::{BTreeMap, BTreeSet};

/// Authenticated primary panel and its embedded measurement engine. Open once
/// for a sequential batch; finish rechecks package bytes before reports leave.
pub struct AssemblyWorkflow {
    directory: PathBuf,
    expected_sha256: String,
    package: VerifiedPackage,
    database: SkaniDatabase,
    /// The frozen definition, or why this package cannot support a genomovar call.
    genomovar: Result<Definition, String>,
    /// Opened on first use, so a batch without a Group A parent never loads it.
    genomovar_database: OnceCell<Result<SkaniDatabase, String>>,
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
        let genomovar = Definition::from_package(&package).map_err(|e| e.to_string());
        Ok(Self {
            directory: directory.to_owned(),
            expected_sha256: expected_sha256.into(),
            package,
            database,
            genomovar,
            genomovar_database: OnceCell::new(),
        })
    }

    /// The frozen genomovar definition this package carries.
    pub fn genomovar_definition(&self) -> Result<&Definition, String> {
        self.genomovar.as_ref().map_err(Clone::clone)
    }

    /// The pinned settings of the genomovar search, read from its database.
    pub fn genomovar_search_parameters(&self) -> Result<String, String> {
        Ok(self
            .genomovar_database()?
            .settings_label(SearchSettings::Genomovar))
    }

    fn genomovar_database(&self) -> Result<&SkaniDatabase, String> {
        self.genomovar_database
            .get_or_init(|| {
                let component = self
                    .package
                    .component(genomovar::COMPONENT_ID)
                    .ok_or("package has no genomovar component")?;
                let parent = Path::new(&component.databases["index.db"].path)
                    .parent()
                    .ok_or("index has no directory")?;
                SkaniDatabase::open(&self.directory.join(parent)).map_err(|e| e.to_string())
            })
            .as_ref()
            .map_err(Clone::clone)
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
        let saved =
            SavedEvidence::from_json(&evidence_value.to_string()).map_err(|e| e.to_string())?;
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
        complete["execution"] = json!({"status": "incomplete", "stage": "genomovar"});
        let section = match &self.genomovar {
            Ok(definition) => self.genomovar(definition, query_label, bytes, threads, &complete),
            Err(why) => Ok(genomovar::before_search(
                &GenomovarParent::from_report(&complete),
                Err(why),
            )
            .expect("an unsupported definition is never searched")),
        };
        let result = match section {
            Ok(section) => {
                complete["genomovar"] = section;
                complete["execution"] = json!({"status": "complete", "stage": "genomovar"});
                Ok(())
            }
            Err(error) => {
                complete["genomovar"] =
                    genomovar::failed(&GenomovarParent::from_report(&complete), &error);
                let error = format!("genomovar search failed: {error}");
                invalidate(&mut complete, "genomovar", &error);
                Err(error)
            }
        };
        *report = complete;
        result
    }

    /// The genomovar section for one query, given the final report of its parent call. The
    /// uncapped genomovar search of the historical chromosome component runs only for an
    /// eligible Group A parent. The front end publishes the report-level `genomovar_id` only
    /// after [`genomovar::recheck`].
    ///
    /// [`Self::analyse`] always passes the frozen definition the package carries; another
    /// definition never yields a supported call through the public command.
    pub fn genomovar(
        &self,
        definition: &Definition,
        query_label: &str,
        bytes: &[u8],
        threads: usize,
        report: &Value,
    ) -> Result<Value, String> {
        let parent = GenomovarParent::from_report(report);
        if let Some(section) = genomovar::before_search(&parent, Ok(definition)) {
            return Ok(section);
        }
        let database = self.genomovar_database()?;
        let component = self
            .package
            .component(genomovar::COMPONENT_ID)
            .ok_or("package has no genomovar component")?;
        let measurement = database
            .search_bytes(
                query_label,
                bytes,
                SearchSettings::Genomovar,
                threads,
                SeedingMode::Auto,
            )
            .map_err(|e| e.to_string())?;
        let by_label: BTreeMap<&str, &str> = component
            .references
            .iter()
            .map(|r| (r.search_label.as_str(), r.reference_id.as_str()))
            .collect();
        let mut seen = BTreeSet::new();
        let mut rows = Vec::new();
        let mut not_returned = Vec::new();
        for row in &measurement.rows {
            let id = *by_label
                .get(row.reference_file.as_str())
                .ok_or("unbound genomovar reference")?;
            if !seen.insert(id) {
                return Err("duplicate genomovar reference".into());
            }
            rows.push(MemberRow {
                reference_id: id.to_owned(),
                ani: row.ani,
                af_query: row.af_query,
                af_reference: row.af_reference,
            });
        }
        for label in &measurement.not_returned_reference_files {
            let id = *by_label
                .get(label.as_str())
                .ok_or("unbound unreturned genomovar reference")?;
            if !seen.insert(id) {
                return Err("duplicate unreturned genomovar reference".into());
            }
            not_returned.push(id);
        }
        if seen.len() != by_label.len() {
            return Err("skani did not assess the complete genomovar component".into());
        }
        let text = |v: &Value| v.as_str().unwrap_or_default().to_owned();
        let context = Context {
            parent,
            input_kind: text(&report["input"]["kind"]),
            query_scope: text(&report["input"]["sequence_scope"]),
            reference_scope: component.sequence_scope.clone(),
            engine: component.engine.clone(),
            engine_version: if component.engine_version == measurement.engine_version {
                measurement.engine_version.into()
            } else {
                format!(
                    "{} measured with {}",
                    component.engine_version, measurement.engine_version
                )
            },
            parameters: database.settings_label(SearchSettings::Genomovar),
            search_status: "completed".into(),
            adapter_verified: true,
            query_sha256: sha256_hex(bytes),
            package_sha256: self.package.package().manifest_sha256.clone(),
            diagnostic_exclusion: false,
        };
        let assessment =
            genomovar::assess(definition, &rows, &context).map_err(|e| e.to_string())?;
        let mut section = assessment.to_value();
        let search = &mut section["search"];
        search["component_id"] = genomovar::COMPONENT_ID.into();
        search["engine_revision"] = measurement.engine_revision.into();
        search["engine_profile"] = measurement.engine_profile.into();
        search["reference_count"] = by_label.len().into();
        search["returned_count"] = rows.len().into();
        search["not_returned_reference_ids"] = json!(not_returned);
        search["member_evidence"] = json!(rows
            .iter()
            .map(
                |r| json!({"reference_id": r.reference_id, "ani_percent": r.ani,
                "aligned_fraction_query_percent": r.af_query,
                "aligned_fraction_reference_percent": r.af_reference})
            )
            .collect::<Vec<_>>());
        Ok(section)
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
            let species = self
                .package
                .concept(&reference.species_id)
                .expect("verified concept");
            let subspecies = reference
                .subspecies_id
                .as_deref()
                .and_then(|id| self.package.concept(id));
            rows.push(json!({"reference_id": reference.reference_id,
                "species_id": species.id, "species_name": species.display_name,
                "subspecies_id": subspecies.map_or("", |c| c.id.as_str()),
                "subspecies_name": subspecies.map_or("", |c| c.display_name.as_str()),
                "lineage_id": "", "reference_role": if reference.type_anchor { "type_anchor" } else { "diversity" },
                "name_status": subspecies.unwrap_or(species).nomenclature_status,
                "ani_percent": row.ani, "aligned_fraction_query_percent": row.af_query,
                "aligned_fraction_reference_percent": row.af_reference,
                "minimum_directional_af_percent": row.af_query.min(row.af_reference)}));
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
            "assignment": {"status": "not_evaluated", "reason": "native_v1_decision_pending",
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

/// Withdraws a report's call after an execution failure at `stage`, keeping its measurement
/// and evidence. An incomplete report never carries an assignment.
pub fn invalidate(report: &mut Value, stage: &str, error: &str) {
    report["execution"] = json!({"status": "incomplete", "stage": stage, "error": error});
    report["assignment"] = json!({"status": "not_evaluated", "species_id": null, "species_name": null,
        "subspecies_id": null, "subspecies_name": null, "lineage_id": null, "genomovar_id": null});
    report["species"] = Value::Null;
    report["subspecies"] = Value::Null;
}
