//! The source manifest records the components, concepts and references the package was built
//! from. The export attestation says the package passed full local validation when it was
//! exported. The portable manifest must list exactly the files the source manifest's recipe names.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{json, Value};

use super::derive::{file_record, COMPONENTS};
use super::manifest::Portable;
use super::tables::TABLES;
use super::{inconsistent, json_record, PackageError};

/// The source manifest, kept whole for the derivation check, with the fields read before it.
pub(super) struct SourceManifest {
    pub(super) value: Value,
    head: Head,
}

#[derive(Deserialize)]
struct Head {
    package_id: String,
    package_version: String,
    release_state: String,
    positive_calling_policy_enabled: bool,
    counts: Value,
    references: Vec<ReferenceHead>,
    verified_artifacts: Vec<Value>,
}

#[derive(Deserialize)]
struct ReferenceHead {
    source: Value,
}

/// The export attestation, which records that the package passed local validation.
#[derive(Debug, Deserialize)]
struct Export {
    local_validation: LocalValidation,
    no_external_distribution: bool,
    no_sequence_or_sketch_recomputation: bool,
    scope: String,
    source_manifest_sha256: String,
}

#[derive(Debug, Deserialize)]
struct LocalValidation {
    counts: Value,
    database_bytes: u64,
    positive_calling_policy_enabled: bool,
    schema_version: String,
    valid: bool,
    verified_artifacts: u64,
}

impl SourceManifest {
    pub(super) fn parse(
        bytes: &[u8],
        flavour: super::flavour::Flavour,
    ) -> Result<Self, PackageError> {
        let value: Value = json_record(bytes, "the source manifest", flavour.source_schema())?;
        let head: Head = serde_json::from_value(value.clone())
            .map_err(|error| inconsistent(format!("the source manifest is malformed: {error}")))?;
        Ok(Self { value, head })
    }

    pub(super) fn check_identity(&self, portable: &Portable) -> Result<(), PackageError> {
        let head = &self.head;
        if head.package_id != portable.package_id
            || head.package_version != portable.package_version
            || head.release_state != portable.release_state
        {
            return Err(inconsistent(
                "the source manifest names another package or version than the portable manifest",
            ));
        }
        if head.positive_calling_policy_enabled {
            return Err(inconsistent("the source manifest enables positive calling"));
        }
        Ok(())
    }

    pub(super) fn check_attestation(
        &self,
        bytes: &[u8],
        portable: &Portable,
    ) -> Result<(), PackageError> {
        let export: Export = serde_json::from_slice(bytes).map_err(|error| {
            inconsistent(format!("the export attestation is malformed: {error}"))
        })?;
        let local = &export.local_validation;
        if !local.valid
            || local.positive_calling_policy_enabled
            || local.schema_version != portable.flavour().source_schema()
        {
            return Err(inconsistent(
                "the export attestation does not record a valid local validation",
            ));
        }
        if export.no_external_distribution != portable.flavour().no_external_distribution()
            || !export.no_sequence_or_sketch_recomputation
            || export.scope != portable.flavour().export_scope()
        {
            return Err(inconsistent("the export attestation has another scope"));
        }
        if export.source_manifest_sha256 != portable.source_manifest.sha256 {
            return Err(inconsistent(
                "the export attestation is for another source manifest",
            ));
        }
        if local.counts != self.head.counts {
            return Err(inconsistent(
                "the export attestation counts differ from the source manifest",
            ));
        }
        if local.verified_artifacts != self.head.verified_artifacts.len() as u64 {
            return Err(inconsistent(
                "the export attestation verified another number of files than the source manifest lists",
            ));
        }
        let database_bytes: u64 = portable
            .assets
            .iter()
            .filter(|asset| asset.roles.iter().any(|role| role.starts_with("database:")))
            .map(|asset| asset.bytes)
            .sum();
        if local.database_bytes != database_bytes {
            return Err(inconsistent(
                "the export attestation counts other database bytes than the manifest lists",
            ));
        }
        Ok(())
    }

    /// The portable listing must be exactly the allowlist the recipe implies: each recipe file
    /// once, at its fixed relative path, with every role it plays, and no sequence or executable.
    pub(super) fn check_listing(&self, assets: &Value) -> Result<(), PackageError> {
        let expected = self.asset_specs()?;
        if *assets != expected {
            return Err(inconsistent(
                "the portable listing differs from the files the recipe names",
            ));
        }
        Ok(())
    }

    fn asset_specs(&self) -> Result<Value, PackageError> {
        let recipe = &self.value["recipe"];
        let mut specs: BTreeMap<String, Value> = BTreeMap::new();
        let mut add = |record: &Value, target: String, role: String| -> Result<(), PackageError> {
            let source = file_record(record)?.to_string();
            match specs.get_mut(&source) {
                Some(existing) => {
                    if existing["sha256"] != record["sha256"]
                        || existing["bytes"] != record["bytes"]
                    {
                        return Err(inconsistent(format!(
                            "{source} is pinned with two different digests or lengths"
                        )));
                    }
                    existing["roles"]
                        .as_array_mut()
                        .expect("roles is a list")
                        .push(role.into());
                }
                None => {
                    specs.insert(
                        source.clone(),
                        json!({
                            "path": target,
                            "source_path": source,
                            "sha256": record["sha256"],
                            "bytes": record["bytes"],
                            "roles": [role],
                        }),
                    );
                }
            }
            Ok(())
        };
        let mut names: Vec<&str> = TABLES.iter().map(|t| t.0).collect();
        names.sort_unstable();
        for name in names {
            add(
                &recipe["source_tables"][name],
                format!("metadata/tables/{name}.tsv"),
                format!("table:{name}"),
            )?;
        }
        for (i, record) in recipe["provenance_files"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            add(
                record,
                format!("metadata/provenance/{i:03}.artifact"),
                format!("provenance:{i}"),
            )?;
        }
        let mut components: Vec<&str> = COMPONENTS.iter().map(|c| c.0).collect();
        components.sort_unstable();
        for id in components {
            let component = &recipe["components"][id];
            add(
                &component["source_list"],
                format!("metadata/lists/{id}.txt"),
                format!("source_list:{id}"),
            )?;
            for (i, record) in component["build_records"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
            {
                add(
                    record,
                    format!("metadata/build/{id}/{i:03}.artifact"),
                    format!("build:{id}:{i}"),
                )?;
            }
            for record in component["database_files"].as_array().into_iter().flatten() {
                let path = file_record(record)?;
                let name = path.rsplit('/').next().unwrap_or(path);
                add(
                    record,
                    format!("databases/{id}/{name}"),
                    format!("database:{id}:{name}"),
                )?;
            }
        }

        let mut excluded: Vec<&Value> = self
            .head
            .references
            .iter()
            .map(|reference| &reference.source["path"])
            .collect();
        for (id, _, _) in COMPONENTS {
            excluded.push(&recipe["components"][id]["skani_binary"]["path"]);
        }
        if excluded
            .iter()
            .filter_map(|path| path.as_str())
            .any(|path| specs.contains_key(path))
        {
            return Err(inconsistent(
                "the recipe would package a sequence or an executable",
            ));
        }
        let mut listed: Vec<Value> = specs.into_values().collect();
        for spec in &mut listed {
            spec["roles"]
                .as_array_mut()
                .expect("roles is a list")
                .sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        }
        listed.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
        let targets: std::collections::BTreeSet<&str> =
            listed.iter().filter_map(|s| s["path"].as_str()).collect();
        if targets.len() != listed.len() {
            return Err(inconsistent("the recipe places two files at one path"));
        }
        Ok(Value::Array(listed))
    }
}
