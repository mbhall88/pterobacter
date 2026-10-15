//! Reference package authentication.
//!
//! A reference package is a directory whose `portable.json` lists its files with their byte
//! lengths and SHA-256 digests. The operator supplies the digest that the manifest must have, and
//! verification refuses any other manifest before it reads the listing. The attested records (the
//! source manifest, the export attestation, the licence and, for a converted package, the version
//! 1 manifest it came from) are verified first because they describe the other files. The listing
//! must be exactly the files the source manifest's recipe names, the directory must hold nothing
//! else, and every listed file is verified. Last, the source manifest must equal the record its
//! packaged tables, recipe and skani indexes derive, so the concepts, references and components the
//! package exposes are the ones its registry gives. Bytes arrive through [`PackageSource`], so this
//! module never opens a path itself.

pub mod anim;
mod derive;
mod flavour;
mod index;
mod manifest;
mod model;
mod resolve;
mod source;
mod tables;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::{self, Read};

use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use manifest::Portable;
use source::SourceManifest;

pub use model::{
    ArchivedFile, Concept, GenomovarDefinition, IndexEntry, PackageCounts, PackageReference, Rank,
    ReferenceComponent,
};

pub const MANIFEST_PATH: &str = "portable.json";

/// Read access to the files of a package and to the names of all of them.
///
/// `open` is only asked for listed paths, which the verifier has checked for safety first, so
/// `path` is relative and free of `..` segments. `Ok(None)` means the package does not hold `path`.
pub trait PackageSource {
    fn open(&self, path: &str) -> io::Result<Option<Box<dyn Read + '_>>>;
    /// Every file the package holds, by relative path.
    fn inventory(&self) -> io::Result<BTreeSet<String>>;
}

/// The category of a rejection, for a front end that maps rejections to exit codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageErrorKind {
    Unexpected,
    Missing,
    Altered,
    Truncated,
    Incompatible,
    Inconsistent,
    Unlisted,
    Unresolved,
    Io,
}

#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("unexpected: {0}")]
    Unexpected(String),
    #[error("missing: {0}")]
    Missing(String),
    #[error("altered: {0}")]
    Altered(String),
    #[error("truncated: {0}")]
    Truncated(String),
    #[error("incompatible: {0}")]
    Incompatible(String),
    #[error("inconsistent: {0}")]
    Inconsistent(String),
    #[error("unlisted: {0}")]
    Unlisted(String),
    #[error("unresolved: {0}")]
    Unresolved(String),
    #[error("could not read {path}: {source}")]
    Io { path: String, source: io::Error },
}

impl PackageError {
    pub fn kind(&self) -> PackageErrorKind {
        match self {
            Self::Unexpected(_) => PackageErrorKind::Unexpected,
            Self::Missing(_) => PackageErrorKind::Missing,
            Self::Altered(_) => PackageErrorKind::Altered,
            Self::Truncated(_) => PackageErrorKind::Truncated,
            Self::Incompatible(_) => PackageErrorKind::Incompatible,
            Self::Inconsistent(_) => PackageErrorKind::Inconsistent,
            Self::Unlisted(_) => PackageErrorKind::Unlisted,
            Self::Unresolved(_) => PackageErrorKind::Unresolved,
            Self::Io { .. } => PackageErrorKind::Io,
        }
    }
}

fn inconsistent(message: impl Into<String>) -> PackageError {
    PackageError::Inconsistent(message.into())
}

fn io_error(path: &str, error: io::Error) -> PackageError {
    PackageError::Io {
        path: path.to_string(),
        source: error,
    }
}

/// Operator-supplied SHA-256 values for registry tables. A table is checked only when its pin is
/// given, and no pin has a built-in default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegistryPins {
    pub named_registry_sha256: Option<String>,
    pub lineage_registry_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PackageIdentity {
    pub id: String,
    pub version: String,
    pub schema_version: String,
    pub release_state: String,
    pub trust_basis: String,
    pub manifest_sha256: String,
    pub source_manifest_sha256: String,
    pub export_validation_sha256: String,
    /// The SHA-256 of the version 1 manifest this package was converted from, if it was.
    pub converted_from_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Licence {
    pub schema_version: String,
    pub package_version: String,
    pub sha256: String,
    pub ncbi_submitter_rights_disclaimer: String,
}

/// A file the manifest lists: where it is in the package, its length and digest, the roles it
/// plays and the archived path it was copied from.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Asset {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub roles: Vec<String>,
    pub source_path: String,
}

/// A package that has passed [`verify_package`], so its fields are private.
///
/// ```compile_fail,E0451
/// use terobacter::reference::VerifiedPackage;
/// let _ = VerifiedPackage { package: todo!(), licence: todo!(), metadata: todo!() };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPackage {
    package: PackageIdentity,
    licence: Licence,
    metadata: model::Metadata,
}

/// Authenticate a package against the SHA-256 its manifest must have. Every listed file must be
/// present with its listed length and digest, the package must hold no unlisted file, and the
/// records that describe the package must agree with one another and with the files.
pub fn verify_package(
    source: &dyn PackageSource,
    expected_sha256: &str,
    pins: &RegistryPins,
) -> Result<VerifiedPackage, PackageError> {
    let manifest_bytes = read_all(source, MANIFEST_PATH)?
        .ok_or_else(|| PackageError::Missing(format!("{MANIFEST_PATH} is not in the package")))?;
    let manifest_sha256 = hex(&Sha256::digest(&manifest_bytes));
    if manifest_sha256 != expected_sha256 {
        return Err(PackageError::Unexpected(format!(
            "{MANIFEST_PATH} has SHA-256 {manifest_sha256}, not the expected {expected_sha256}"
        )));
    }
    let manifest = Portable::parse(&manifest_bytes)?;
    let raw: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| inconsistent(format!("{MANIFEST_PATH} is not JSON: {error}")))?;
    let attested: Vec<Asset> = manifest.attested().iter().map(|r| r.as_asset()).collect();
    let listed: Vec<&Asset> = manifest.assets.iter().chain(&attested).collect();
    check_listings(&listed)?;

    let mut held: BTreeMap<&str, Vec<u8>> = BTreeMap::new();
    for record in &attested {
        held.insert(record.path.as_str(), verify_file(source, record, true)?);
    }
    if let Some(record) = &manifest.converted_from {
        manifest.check_conversion(&held[record.path.as_str()], &raw)?;
    }
    let source_manifest = SourceManifest::parse(
        &held[manifest.source_manifest.path.as_str()],
        manifest.flavour(),
    )?;
    source_manifest.check_identity(&manifest)?;
    source_manifest
        .check_attestation(&held[manifest.export_validation.path.as_str()], &manifest)?;
    source_manifest.check_listing(&raw["assets"])?;
    check_inventory(source, &listed)?;

    for asset in &manifest.assets {
        let keep = manifest::is_held(asset);
        let bytes = verify_file(source, asset, keep)?;
        if keep {
            held.insert(asset.path.as_str(), bytes);
        }
    }
    let licence = manifest::licence(&held[manifest.licence.path.as_str()], &manifest)?;
    check_pins(&manifest.assets, pins)?;
    let tables = tables::Tables::load(&manifest.assets, &held)?;
    derive::check(
        &source_manifest.value,
        &tables,
        &manifest.assets,
        &held,
        manifest.flavour(),
    )?;
    let metadata = model::Metadata::from_checked(&source_manifest.value, &manifest.assets)?;

    Ok(VerifiedPackage {
        package: PackageIdentity {
            id: manifest.package_id.clone(),
            version: manifest.package_version.clone(),
            schema_version: manifest.schema_version.clone(),
            release_state: manifest.release_state.clone(),
            trust_basis: manifest.trust_basis.clone(),
            manifest_sha256,
            source_manifest_sha256: manifest.source_manifest.sha256.clone(),
            export_validation_sha256: manifest.export_validation.sha256.clone(),
            converted_from_sha256: manifest.converted_from.as_ref().map(|r| r.sha256.clone()),
        },
        licence,
        metadata,
    })
}

impl VerifiedPackage {
    pub fn package(&self) -> &PackageIdentity {
        &self.package
    }

    pub fn licence(&self) -> &Licence {
        &self.licence
    }

    pub fn components(&self) -> &[ReferenceComponent] {
        &self.metadata.components
    }

    pub fn component(&self, id: &str) -> Option<&ReferenceComponent> {
        self.metadata
            .components
            .iter()
            .find(|component| component.id == id)
    }

    /// Every concept, in ID order, including context lineages and genomovars.
    pub fn concepts(&self) -> &[Concept] {
        &self.metadata.concepts
    }

    pub fn concept(&self, id: &str) -> Option<&Concept> {
        self.metadata
            .concepts
            .binary_search_by(|concept| concept.id.as_str().cmp(id))
            .ok()
            .map(|at| &self.metadata.concepts[at])
    }

    /// Every reference scope record of both components.
    pub fn references(&self) -> impl Iterator<Item = &PackageReference> {
        self.metadata
            .components
            .iter()
            .flat_map(|component| &component.references)
    }

    pub fn reference(&self, id: &str) -> Option<&PackageReference> {
        self.metadata
            .components
            .iter()
            .find_map(|component| component.reference(id))
    }

    /// The historical type crosswalk rows, in taxon-name order.
    pub fn historical_types(&self) -> &[BTreeMap<String, String>] {
        &self.metadata.historical_types
    }

    /// The development context records that are deliberately not packaged references.
    pub fn context_records(&self) -> &[BTreeMap<String, String>] {
        &self.metadata.context_records
    }

    /// Cohort rows by coverage category, as the source records them.
    pub fn cohort_coverage_summary(&self) -> &BTreeMap<String, u64> {
        &self.metadata.cohort_coverage_summary
    }

    pub fn counts(&self) -> &PackageCounts {
        &self.metadata.counts
    }

    pub fn to_value(&self) -> Value {
        let components: Map<String, Value> = self
            .metadata
            .components
            .iter()
            .map(|component| (component.id.clone(), component_value(component)))
            .collect();
        let counts = &self.metadata.counts;
        json!({
            "status": "verified",
            "package": {
                "id": self.package.id,
                "version": self.package.version,
                "schema_version": self.package.schema_version,
                "release_state": self.package.release_state,
                "trust_basis": self.package.trust_basis,
                "manifest_sha256": self.package.manifest_sha256,
                "source_manifest_sha256": self.package.source_manifest_sha256,
                "export_validation_sha256": self.package.export_validation_sha256,
                "converted_from_sha256": self.package.converted_from_sha256,
            },
            "licence": {
                "schema_version": self.licence.schema_version,
                "package_version": self.licence.package_version,
                "sha256": self.licence.sha256,
                "ncbi_submitter_rights_disclaimer": self.licence.ncbi_submitter_rights_disclaimer,
            },
            "counts": {
                "primary_whole_references": counts.primary_whole_references,
                "historical_chromosome_definition_references": counts.historical_chromosome_definition_references,
                "unique_accessions": counts.unique_accessions,
                "reference_scope_records": counts.reference_scope_records,
                "named_concepts": counts.named_concepts,
                "context_lineage_concepts": counts.context_lineage_concepts,
                "genomovar_concepts": counts.genomovar_concepts,
                "development_context_records_not_packaged": counts.development_context_records_not_packaged,
                "eligible_cohort_records": counts.eligible_cohort_records,
                "eligible_species_reference_represented": counts.eligible_species_reference_represented,
                "primary_historical_type_accessions": counts.primary_historical_type_accessions,
            },
            "cohort_coverage_summary": self.metadata.cohort_coverage_summary,
            "components": components,
        })
    }
}

fn component_value(component: &ReferenceComponent) -> Value {
    let databases: Map<String, Value> = component
        .databases
        .iter()
        .map(|(file, asset)| (file.clone(), asset_value(asset)))
        .collect();
    let builds: Vec<Value> = component.builds.iter().map(asset_value).collect();
    json!({
        "reference_role": component.reference_role,
        "sequence_scope": component.sequence_scope,
        "engine": component.engine,
        "engine_version": component.engine_version,
        "database_format": component.database_format,
        "reference_count": component.references.len(),
        "databases": databases,
        "builds": builds,
    })
}

fn asset_value(asset: &Asset) -> Value {
    json!({
        "path": asset.path,
        "bytes": asset.bytes,
        "sha256": asset.sha256,
        "roles": asset.roles,
    })
}

/// Parse a JSON record whose schema must be exactly `schema`. A record of another schema is
/// incompatible; one with no schema, or one that does not fit the type, is inconsistent.
fn json_record<T: DeserializeOwned>(
    bytes: &[u8],
    name: &str,
    schema: &str,
) -> Result<T, PackageError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| inconsistent(format!("{name} is not JSON: {error}")))?;
    match value.get("schema_version").and_then(Value::as_str) {
        Some(found) if found == schema => {}
        Some(found) => {
            return Err(PackageError::Incompatible(format!(
                "{name} has schema {found}, not {schema}"
            )))
        }
        None => return Err(inconsistent(format!("{name} has no schema_version"))),
    }
    serde_json::from_value(value)
        .map_err(|error| inconsistent(format!("{name} is malformed: {error}")))
}

fn role_asset<'a>(assets: &'a [Asset], role: &str) -> Result<&'a Asset, PackageError> {
    let mut matches = assets
        .iter()
        .filter(|asset| asset.roles.iter().any(|r| r.as_str() == role));
    let first = matches
        .next()
        .ok_or_else(|| inconsistent(format!("the manifest has no asset with role {role}")))?;
    if matches.next().is_some() {
        return Err(inconsistent(format!(
            "the manifest has more than one asset with role {role}"
        )));
    }
    Ok(first)
}

fn check_listings(assets: &[&Asset]) -> Result<(), PackageError> {
    let mut seen = HashSet::new();
    for asset in assets {
        if !is_safe_path(&asset.path) || asset.path == MANIFEST_PATH {
            return Err(inconsistent(format!(
                "the manifest lists an unsafe path: {}",
                asset.path
            )));
        }
        if !seen.insert(asset.path.as_str()) {
            return Err(inconsistent(format!(
                "{} is listed more than once",
                asset.path
            )));
        }
        if !is_sha256(&asset.sha256) {
            return Err(inconsistent(format!(
                "{} has no valid SHA-256 digest",
                asset.path
            )));
        }
    }
    Ok(())
}

fn check_inventory(source: &dyn PackageSource, listed: &[&Asset]) -> Result<(), PackageError> {
    let present = source.inventory().map_err(|error| io_error(".", error))?;
    let mut known: HashSet<&str> = listed.iter().map(|asset| asset.path.as_str()).collect();
    known.insert(MANIFEST_PATH);
    match present.iter().find(|path| !known.contains(path.as_str())) {
        Some(path) => Err(PackageError::Unlisted(format!(
            "{path} is in the package but not in the manifest"
        ))),
        None => Ok(()),
    }
}

fn check_pins(assets: &[Asset], pins: &RegistryPins) -> Result<(), PackageError> {
    let wanted = [
        (
            "table:named_registry",
            pins.named_registry_sha256.as_deref(),
        ),
        (
            "table:lineage_registry",
            pins.lineage_registry_sha256.as_deref(),
        ),
    ];
    for (role, pin) in wanted {
        let Some(pin) = pin else { continue };
        let asset = assets
            .iter()
            .find(|asset| asset.roles.iter().any(|r| r.as_str() == role))
            .ok_or_else(|| {
                PackageError::Missing(format!("{role} is not listed in the manifest"))
            })?;
        if asset.sha256 != pin {
            return Err(PackageError::Unexpected(format!(
                "{} has SHA-256 {}, not the pinned {pin}",
                asset.path, asset.sha256
            )));
        }
    }
    Ok(())
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

pub(crate) fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
