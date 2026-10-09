//! The portable manifest, the records it attests, and the licence manifest.
//!
//! Version 2 adds a licence manifest to the version 1 portable package. A package converted from
//! version 1 also carries the exact version 1 manifest it was converted from, so evidence computed
//! against that version 1 package can be traced to the same files.

use serde::Deserialize;
use serde_json::Value;

use super::{inconsistent, json_record, Asset, Licence, PackageError, MANIFEST_PATH};

pub(super) const PACKAGE_SCHEMA: &str = "2.0.0-dev-portable-reference-package";
pub(super) const V1_PACKAGE_SCHEMA: &str = "1.0.0-dev-portable-reference-package";
pub(super) const LICENCE_SCHEMA: &str = "2.0.0-reference-licence-manifest";
pub(super) const RELEASE_STATE: &str = "development_proposal_not_distributable";
pub(super) const TRUST_BASIS: &str =
    "expected_package_digest_and_export_attestation_not_authenticated_release";

/// A file the manifest attests by its own entry rather than in `assets`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    pub(super) path: String,
    pub(super) bytes: u64,
    pub(super) sha256: String,
}

impl Record {
    pub(super) fn as_asset(&self) -> Asset {
        Asset {
            path: self.path.clone(),
            bytes: self.bytes,
            sha256: self.sha256.clone(),
            roles: Vec::new(),
            source_path: String::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Portable {
    #[allow(dead_code)] // checked by json_record before the rest is read
    schema_version: String,
    pub(super) package_id: String,
    pub(super) package_version: String,
    pub(super) release_state: String,
    pub(super) trust_basis: String,
    pub(super) positive_calling_policy_enabled: bool,
    pub(super) source_manifest: Record,
    pub(super) export_validation: Record,
    pub(super) licence: Record,
    #[serde(default)]
    pub(super) converted_from: Option<Record>,
    pub(super) assets: Vec<Asset>,
}

impl Portable {
    pub(super) fn parse(bytes: &[u8]) -> Result<Self, PackageError> {
        let portable: Self = json_record(bytes, MANIFEST_PATH, PACKAGE_SCHEMA)?;
        if portable.release_state != RELEASE_STATE || portable.trust_basis != TRUST_BASIS {
            return Err(inconsistent(format!(
                "{MANIFEST_PATH} names another release state or trust basis"
            )));
        }
        if portable.positive_calling_policy_enabled {
            return Err(inconsistent(format!(
                "{MANIFEST_PATH} enables positive calling"
            )));
        }
        Ok(portable)
    }

    /// The records attested outside `assets`, in the order they are verified.
    pub(super) fn attested(&self) -> Vec<&Record> {
        [
            Some(&self.source_manifest),
            Some(&self.export_validation),
            Some(&self.licence),
            self.converted_from.as_ref(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    /// A version 1 manifest this package was converted from must describe the same package with
    /// the same files: only the schema, the licence and this record may differ.
    pub(super) fn check_conversion(&self, bytes: &[u8], raw: &Value) -> Result<(), PackageError> {
        let v1: Value = serde_json::from_slice(bytes).map_err(|error| {
            inconsistent(format!("the converted-from manifest is not JSON: {error}"))
        })?;
        let Some(fields) = v1.as_object() else {
            return Err(inconsistent("the converted-from manifest is not an object"));
        };
        if fields.get("schema_version").and_then(Value::as_str) != Some(V1_PACKAGE_SCHEMA) {
            return Err(PackageError::Incompatible(format!(
                "the converted-from manifest is not schema {V1_PACKAGE_SCHEMA}"
            )));
        }
        let shared = [
            "package_id",
            "package_version",
            "release_state",
            "trust_basis",
            "positive_calling_policy_enabled",
            "source_manifest",
            "export_validation",
            "assets",
        ];
        if fields.len() != shared.len() + 1 {
            return Err(inconsistent(
                "the converted-from manifest has missing or unknown fields",
            ));
        }
        if let Some(field) = shared
            .iter()
            .find(|key| fields.get(**key) != raw.get(**key))
        {
            return Err(inconsistent(format!(
                "the converted-from manifest has another {field} than this package"
            )));
        }
        Ok(())
    }
}

/// Whether the verifier needs the bytes of a listed file after checking it: the metadata, and
/// each database's index, which names the sequences the sketches were built from.
pub(super) fn is_held(asset: &Asset) -> bool {
    let parts: Vec<&str> = asset.path.split('/').collect();
    parts.first() == Some(&"metadata") || matches!(parts.as_slice(), ["databases", _, "index.db"])
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LicenceRecord {
    #[allow(dead_code)] // checked by json_record before the rest is read
    schema_version: String,
    package_version: String,
    source_manifest_sha256: String,
    ncbi_submitter_rights_disclaimer: String,
}

pub(super) fn licence(bytes: &[u8], portable: &Portable) -> Result<Licence, PackageError> {
    let record: LicenceRecord = json_record(bytes, "the licence manifest", LICENCE_SCHEMA)?;
    if record.package_version != portable.package_version {
        return Err(inconsistent(format!(
            "the licence manifest is for version {}, not {}",
            record.package_version, portable.package_version
        )));
    }
    if record.source_manifest_sha256 != portable.source_manifest.sha256 {
        return Err(inconsistent(
            "the licence manifest names another source manifest",
        ));
    }
    if record.ncbi_submitter_rights_disclaimer.trim().is_empty() {
        return Err(inconsistent(
            "the licence manifest has no NCBI submitter-rights disclaimer",
        ));
    }
    Ok(Licence {
        schema_version: LICENCE_SCHEMA.to_string(),
        package_version: record.package_version,
        sha256: portable.licence.sha256.clone(),
        ncbi_submitter_rights_disclaimer: record.ncbi_submitter_rights_disclaimer,
    })
}
