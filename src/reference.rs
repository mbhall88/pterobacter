//! Reference package authentication.
//!
//! A reference package is a directory whose `portable.json` lists files with their byte lengths
//! and SHA-256 digests. Verification checks every listed file against that listing, and checks
//! that the listing describes both components and a licence manifest. Bytes arrive through
//! [`PackageSource`], so this module never opens a path itself. Files the listing does not name
//! are not inspected.

use std::collections::{BTreeMap, HashSet};
use std::io::{self, Read};

use serde::Deserialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::evidence::Provenance;

pub const PACKAGE_SCHEMA: &str = "1.0.0-dev-portable-reference-package";
pub const LICENCE_SCHEMA: &str = "1.0.0-reference-licence-manifest";
pub const MANIFEST_PATH: &str = "portable.json";
const COMPONENTS: [&str; 2] = ["primary_whole", "genomovar_chromosome"];
const DATABASE_FILES: [&str; 3] = ["index.db", "markers.bin", "sketches.db"];

/// Read access to the files of a package. `Ok(None)` means the package does not contain `path`.
/// The verifier checks each listed path for safety before asking for it, so `path` is always
/// relative and free of `..` segments.
pub trait PackageSource {
    fn open(&self, path: &str) -> io::Result<Option<Box<dyn Read + '_>>>;
}

/// The category of a rejection, for a front end that maps rejections to exit codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageErrorKind {
    Missing,
    Altered,
    Truncated,
    Incompatible,
    Incomplete,
    NotBound,
    Io,
}

#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("missing: {0}")]
    Missing(String),
    #[error("altered: {0}")]
    Altered(String),
    #[error("truncated: {0}")]
    Truncated(String),
    #[error("incompatible: {0}")]
    Incompatible(String),
    #[error("incomplete: {0}")]
    Incomplete(String),
    #[error("not bound: {0}")]
    NotBound(String),
    #[error("could not read {path}: {source}")]
    Io { path: String, source: io::Error },
}

impl PackageError {
    pub fn kind(&self) -> PackageErrorKind {
        match self {
            Self::Missing(_) => PackageErrorKind::Missing,
            Self::Altered(_) => PackageErrorKind::Altered,
            Self::Truncated(_) => PackageErrorKind::Truncated,
            Self::Incompatible(_) => PackageErrorKind::Incompatible,
            Self::Incomplete(_) => PackageErrorKind::Incomplete,
            Self::NotBound(_) => PackageErrorKind::NotBound,
            Self::Io { .. } => PackageErrorKind::Io,
        }
    }
}

fn incomplete(message: impl Into<String>) -> PackageError {
    PackageError::Incomplete(message.into())
}

fn io_error(path: &str, error: io::Error) -> PackageError {
    PackageError::Io {
        path: path.to_string(),
        source: error,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageIdentity {
    pub id: String,
    pub version: String,
    pub release_state: String,
    pub trust_basis: String,
    pub manifest_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Licence {
    pub schema_version: String,
    pub package_version: String,
    pub sha256: String,
    pub ncbi_submitter_rights_disclaimer: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Asset {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    #[serde(default)]
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    pub id: String,
    /// Keyed by file name: `index.db`, `markers.bin` and `sketches.db`.
    pub databases: BTreeMap<String, Asset>,
    pub builds: Vec<Asset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPackage {
    pub package: PackageIdentity,
    pub licence: Licence,
    pub components: Vec<Component>,
}

#[derive(Deserialize)]
struct Manifest {
    package_id: String,
    package_version: String,
    release_state: String,
    trust_basis: String,
    assets: Vec<Asset>,
    export_validation: Asset,
    source_manifest: Asset,
}

#[derive(Deserialize)]
struct LicenceManifest {
    schema_version: String,
    package_version: String,
    ncbi_submitter_rights_disclaimer: String,
}

/// Authenticate a package: every listed file must be present with its listed length and digest,
/// the components and licence manifest must be complete, and the schema must be the one this
/// build reads.
pub fn verify_package(source: &dyn PackageSource) -> Result<VerifiedPackage, PackageError> {
    let manifest_bytes = read_all(source, MANIFEST_PATH)?
        .ok_or_else(|| PackageError::Missing(format!("{MANIFEST_PATH} is not in the package")))?;
    let manifest = parse_manifest(&manifest_bytes)?;

    let all_assets: Vec<&Asset> = manifest
        .assets
        .iter()
        .chain([&manifest.export_validation, &manifest.source_manifest])
        .collect();
    check_listings(&all_assets)?;
    let (components, licence_asset) = components_and_licence(&manifest.assets)?;

    let mut licence_bytes = Vec::new();
    for asset in &all_assets {
        if asset.path == licence_asset.path {
            licence_bytes = verify_file(source, asset, true)?;
        } else {
            verify_file(source, asset, false)?;
        }
    }
    let licence = parse_licence(
        &licence_bytes,
        &manifest.package_version,
        licence_asset.sha256,
    )?;

    Ok(VerifiedPackage {
        package: PackageIdentity {
            id: manifest.package_id,
            version: manifest.package_version,
            release_state: manifest.release_state,
            trust_basis: manifest.trust_basis,
            manifest_sha256: hex(&Sha256::digest(&manifest_bytes)),
        },
        licence,
        components,
    })
}

impl VerifiedPackage {
    pub fn component(&self, id: &str) -> Option<&Component> {
        self.components.iter().find(|component| component.id == id)
    }

    /// Evidence is bound to a package by the digest of its manifest.
    pub fn check_binding(&self, provenance: &Provenance) -> Result<(), PackageError> {
        if provenance.reference_package_sha256 == self.package.manifest_sha256 {
            return Ok(());
        }
        Err(PackageError::NotBound(format!(
            "the evidence was searched against package {}, not the verified package {}",
            provenance.reference_package_sha256, self.package.manifest_sha256
        )))
    }

    pub fn to_value(&self) -> Value {
        let components: Map<String, Value> = self
            .components
            .iter()
            .map(|component| (component.id.clone(), component.to_value()))
            .collect();
        json!({
            "status": "verified",
            "package": {
                "id": self.package.id,
                "version": self.package.version,
                "release_state": self.package.release_state,
                "trust_basis": self.package.trust_basis,
                "manifest_sha256": self.package.manifest_sha256,
            },
            "licence": {
                "schema_version": self.licence.schema_version,
                "package_version": self.licence.package_version,
                "sha256": self.licence.sha256,
                "ncbi_submitter_rights_disclaimer": self.licence.ncbi_submitter_rights_disclaimer,
            },
            "components": components,
        })
    }
}

impl Component {
    fn to_value(&self) -> Value {
        let databases: Map<String, Value> = self
            .databases
            .iter()
            .map(|(file, asset)| (file.clone(), asset_value(asset)))
            .collect();
        let builds: Vec<Value> = self.builds.iter().map(asset_value).collect();
        json!({ "databases": databases, "builds": builds })
    }
}

fn asset_value(asset: &Asset) -> Value {
    json!({
        "path": asset.path,
        "bytes": asset.bytes,
        "sha256": asset.sha256,
        "roles": asset.roles,
    })
}

fn parse_manifest(bytes: &[u8]) -> Result<Manifest, PackageError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| incomplete(format!("{MANIFEST_PATH} is not JSON: {error}")))?;
    match value.get("schema_version").and_then(Value::as_str) {
        Some(PACKAGE_SCHEMA) => {}
        found => {
            return Err(PackageError::Incompatible(format!(
                "expected package schema {PACKAGE_SCHEMA}, found {}",
                found.unwrap_or("none")
            )))
        }
    }
    serde_json::from_value(value)
        .map_err(|error| incomplete(format!("{MANIFEST_PATH} is incomplete: {error}")))
}

fn check_listings(assets: &[&Asset]) -> Result<(), PackageError> {
    let mut seen = HashSet::new();
    for asset in assets {
        if !is_safe_path(&asset.path) || asset.path == MANIFEST_PATH {
            return Err(incomplete(format!(
                "the manifest lists an unsafe path: {}",
                asset.path
            )));
        }
        if !seen.insert(asset.path.as_str()) {
            return Err(incomplete(format!(
                "{} is listed more than once",
                asset.path
            )));
        }
        if !is_sha256(&asset.sha256) {
            return Err(incomplete(format!(
                "{} has no valid SHA-256 digest",
                asset.path
            )));
        }
    }
    Ok(())
}

/// Group the assets into the two components and find the one licence manifest.
fn components_and_licence(assets: &[Asset]) -> Result<(Vec<Component>, Asset), PackageError> {
    let mut by_id: BTreeMap<String, Component> = COMPONENTS
        .iter()
        .map(|id| {
            let component = Component {
                id: id.to_string(),
                databases: BTreeMap::new(),
                builds: Vec::new(),
            };
            (id.to_string(), component)
        })
        .collect();
    let mut licence: Option<Asset> = None;

    for asset in assets {
        for role in &asset.roles {
            let parts: Vec<&str> = role.split(':').collect();
            match parts.as_slice() {
                ["database", id, file] if DATABASE_FILES.contains(file) => {
                    let component = component_entry(&mut by_id, id, role)?;
                    if component
                        .databases
                        .insert(file.to_string(), asset.clone())
                        .is_some()
                    {
                        return Err(incomplete(format!("{role} is listed more than once")));
                    }
                }
                ["database", _, _] => {
                    return Err(incomplete(format!("{role} names an unknown database file")))
                }
                ["build", id, _] => {
                    component_entry(&mut by_id, id, role)?
                        .builds
                        .push(asset.clone());
                }
                ["licence", "manifest"] => {
                    let previous = licence.replace(asset.clone());
                    if previous.is_some() {
                        return Err(incomplete("the package has more than one licence manifest"));
                    }
                }
                _ => {}
            }
        }
    }

    let mut components = Vec::with_capacity(COMPONENTS.len());
    for id in COMPONENTS {
        let component = by_id.remove(id).expect("every component was inserted");
        for file in DATABASE_FILES {
            if !component.databases.contains_key(file) {
                return Err(incomplete(format!("component {id} has no database {file}")));
            }
        }
        if component.builds.is_empty() {
            return Err(incomplete(format!("component {id} has no build record")));
        }
        components.push(component);
    }
    let licence = licence.ok_or_else(|| incomplete("the package has no licence manifest"))?;
    Ok((components, licence))
}

fn component_entry<'a>(
    by_id: &'a mut BTreeMap<String, Component>,
    id: &str,
    role: &str,
) -> Result<&'a mut Component, PackageError> {
    by_id
        .get_mut(id)
        .ok_or_else(|| incomplete(format!("{role} names an unknown component")))
}

/// Check one listed file and return its bytes when `keep` is set. A file shorter than listed is
/// truncated; a longer one, or one of the right length with a different digest, is altered.
fn verify_file(
    source: &dyn PackageSource,
    asset: &Asset,
    keep: bool,
) -> Result<Vec<u8>, PackageError> {
    let mut reader = source
        .open(&asset.path)
        .map_err(|error| io_error(&asset.path, error))?
        .ok_or_else(|| {
            PackageError::Missing(format!(
                "{} is listed in the manifest but is not in the package",
                asset.path
            ))
        })?;
    let mut hasher = Sha256::new();
    let mut kept = Vec::new();
    let mut actual = 0u64;
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| io_error(&asset.path, error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        actual += read as u64;
        if keep {
            kept.extend_from_slice(&buffer[..read]);
        }
    }
    if actual < asset.bytes {
        return Err(PackageError::Truncated(format!(
            "{} has {actual} of its {} listed bytes",
            asset.path, asset.bytes
        )));
    }
    if actual > asset.bytes || hex(&hasher.finalize()) != asset.sha256 {
        return Err(PackageError::Altered(format!(
            "{} does not match its listed length and SHA-256",
            asset.path
        )));
    }
    Ok(kept)
}

fn parse_licence(
    bytes: &[u8],
    package_version: &str,
    sha256: String,
) -> Result<Licence, PackageError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| incomplete(format!("licence manifest is not JSON: {error}")))?;
    match value.get("schema_version").and_then(Value::as_str) {
        Some(LICENCE_SCHEMA) => {}
        found => {
            return Err(PackageError::Incompatible(format!(
                "expected licence schema {LICENCE_SCHEMA}, found {}",
                found.unwrap_or("none")
            )))
        }
    }
    let licence: LicenceManifest = serde_json::from_value(value)
        .map_err(|error| incomplete(format!("licence manifest is incomplete: {error}")))?;
    if licence.package_version != package_version {
        return Err(incomplete(format!(
            "licence manifest is for version {}, not {package_version}",
            licence.package_version
        )));
    }
    if licence.ncbi_submitter_rights_disclaimer.trim().is_empty() {
        return Err(incomplete(
            "licence manifest has no NCBI submitter-rights disclaimer",
        ));
    }
    Ok(Licence {
        schema_version: licence.schema_version,
        package_version: licence.package_version,
        sha256,
        ncbi_submitter_rights_disclaimer: licence.ncbi_submitter_rights_disclaimer,
    })
}

fn read_all(source: &dyn PackageSource, path: &str) -> Result<Option<Vec<u8>>, PackageError> {
    let Some(mut reader) = source.open(path).map_err(|error| io_error(path, error))? else {
        return Ok(None);
    };
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| io_error(path, error))?;
    Ok(Some(bytes))
}

fn is_safe_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 512
        && path.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && segment
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        })
}

fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
