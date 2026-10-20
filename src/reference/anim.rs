//! Optional whole-assembly FASTA component, authenticated through the package source port.
use super::{PackageReference, PackageSource, VerifiedPackage};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
};

pub const ANIM_MANIFEST: &str = "anim.json";
const MAX_FASTA_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    component_id: String,
    sequence_scope: String,
    reference_package_sha256: String,
    licence: FilePin,
    references: Vec<FastaReference>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FilePin {
    path: String,
    sha256: String,
    bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FastaReference {
    pub reference_id: String,
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    pub concept_ids: Vec<String>,
}

/// Complete primary FASTA inventory bound to an authenticated skani package.
#[derive(Debug)]
pub struct AnimReference {
    pub manifest_sha256: String,
    pub reference_package_sha256: String,
    pub licence_sha256: String,
    references: BTreeMap<String, FastaReference>,
}
impl AnimReference {
    pub fn reference(&self, id: &str) -> Option<&FastaReference> {
        self.references.get(id)
    }
}

/// Concept context in the same deterministic order used by native pair verification.
pub fn anim_concept_ids(reference: &PackageReference) -> Vec<String> {
    let mut ids = vec![reference.species_id.clone()];
    if let Some(id) = &reference.subspecies_id {
        ids.push(id.clone());
    }
    ids
}

fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 512
        && path.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && segment
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        })
}
fn check_file(source: &dyn PackageSource, pin: &FilePin) -> Result<(), String> {
    if !safe_path(&pin.path)
        || !super::is_sha256(&pin.sha256)
        || pin.bytes == 0
        || pin.bytes > MAX_FASTA_BYTES
    {
        return Err(format!("{}: invalid FASTA component file pin", pin.path));
    }
    let mut reader = source
        .open(&pin.path)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("missing component file {}", pin.path))?;
    let mut digest = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0; 65536];
    loop {
        let n = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        size += n as u64;
        if size > pin.bytes {
            return Err(format!("{}: component length mismatch", pin.path));
        }
        digest.update(&buffer[..n]);
    }
    if size != pin.bytes || crate::hex::encode(&digest.finalize()) != pin.sha256 {
        return Err(format!("{}: component bytes/hash mismatch", pin.path));
    }
    Ok(())
}

/// Verify every reference, including unreturned competitors, and the primary licence copy.
/// Extra files, missing IDs, changed concepts or chromosome replacements are refused.
pub fn verify_anim_reference(
    source: &dyn PackageSource,
    expected: &str,
    package: &VerifiedPackage,
) -> Result<AnimReference, String> {
    if !super::is_sha256(expected) {
        return Err("invalid ANIm manifest SHA-256".into());
    }
    let reader = source
        .open(ANIM_MANIFEST)
        .map_err(|e| e.to_string())?
        .ok_or("missing anim.json")?;
    let mut bytes = Vec::new();
    reader
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 || crate::evidence::sha256_hex(&bytes) != expected {
        return Err("ANIm manifest size/hash mismatch".into());
    }
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if manifest.schema != "terobacter.anim-reference/1"
        || manifest.component_id != "primary_whole"
        || manifest.sequence_scope != "whole_assembly_as_deposited"
        || manifest.reference_package_sha256 != package.package().manifest_sha256
    {
        return Err("ANIm component does not describe this whole-assembly primary package".into());
    }
    if manifest.licence.sha256 != package.licence().sha256 {
        return Err("ANIm component licence differs from primary package".into());
    }
    let primary = package
        .component("primary_whole")
        .ok_or("missing primary component")?;
    if manifest.references.len() != primary.references.len() {
        return Err("ANIm component is not the complete primary panel".into());
    }
    let mut paths = BTreeSet::from([ANIM_MANIFEST.to_owned()]);
    if !paths.insert(manifest.licence.path.clone()) {
        return Err("duplicate component path".into());
    }
    check_file(source, &manifest.licence)?;
    let mut references = BTreeMap::new();
    for reference in manifest.references {
        let current = primary
            .reference(&reference.reference_id)
            .ok_or("ANIm reference outside primary panel")?;
        if reference.sha256 != current.source.sha256
            || reference.bytes != current.source.bytes
            || reference.concept_ids != anim_concept_ids(current)
        {
            return Err(format!(
                "{}: ANIm source/concept mismatch",
                reference.reference_id
            ));
        }
        if !paths.insert(reference.path.clone()) || references.contains_key(&reference.reference_id)
        {
            return Err("duplicate ANIm reference/path".into());
        }
        check_file(
            source,
            &FilePin {
                path: reference.path.clone(),
                sha256: reference.sha256.clone(),
                bytes: reference.bytes,
            },
        )?;
        references.insert(reference.reference_id.clone(), reference);
    }
    if source.inventory().map_err(|e| e.to_string())? != paths {
        return Err("ANIm component inventory differs from manifest".into());
    }
    Ok(AnimReference {
        manifest_sha256: expected.into(),
        reference_package_sha256: manifest.reference_package_sha256,
        licence_sha256: manifest.licence.sha256,
        references,
    })
}
