//! The portable manifest, the three attested records it names, and the licence manifest.

use serde::Deserialize;

use super::{inconsistent, json_record, Asset, Licence, PackageError, MANIFEST_PATH};

pub(super) const PACKAGE_SCHEMA: &str = "2.0.0-dev-portable-reference-package";
pub(super) const LICENCE_SCHEMA: &str = "2.0.0-reference-licence-manifest";
pub(super) const RELEASE_STATE: &str = "development_proposal_not_distributable";
pub(super) const TRUST_BASIS: &str =
    "expected_package_digest_and_export_attestation_not_authenticated_release";

#[derive(Debug, Deserialize)]
pub(super) struct Portable {
    pub(super) package_id: String,
    pub(super) package_version: String,
    pub(super) release_state: String,
    pub(super) trust_basis: String,
    pub(super) positive_calling_policy_enabled: bool,
    pub(super) source_manifest: Asset,
    pub(super) export_validation: Asset,
    pub(super) licence: Asset,
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

    pub(super) fn attested(&self) -> [&Asset; 3] {
        [
            &self.source_manifest,
            &self.export_validation,
            &self.licence,
        ]
    }
}

/// Whether the verifier needs the bytes of a listed file after checking it: the metadata, and
/// each database's index, which names the sequences the sketches were built from.
pub(super) fn is_held(asset: &Asset) -> bool {
    let parts: Vec<&str> = asset.path.split('/').collect();
    parts.first() == Some(&"metadata") || matches!(parts.as_slice(), ["databases", _, "index.db"])
}

#[derive(Deserialize)]
struct LicenceRecord {
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
