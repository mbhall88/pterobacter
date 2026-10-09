//! The reference package used by the integration tests: the committed v2 fixture, loaded into
//! memory so a test can damage exactly one thing and check the rejection that follows.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use terobacter::reference::{
    verify_package, PackageError, PackageSource, RegistryPins, VerifiedPackage,
};

pub const FIXTURE_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/reference-v2/package"
);
pub const PINNED_MANIFEST_SHA256: &str =
    "d594ff403cf479ce677b39ff018cdceb3e5f08582610c6f519e927c16de9f797";
/// The version 1 manifest the fixture was converted from.
pub const CONVERTED_FROM_SHA256: &str =
    "0a48da33dc2adef2feefa592ef7a9ed9e15a184168231bb8ca826f2df80f3b28";
pub const PACKAGE_VERSION: &str = "2026-10-09.test2";
pub const DISCLAIMER: &str =
    "Synthetic test disclaimer. A real package carries the NCBI molecular data usage statement.";
pub const MANIFEST: &str = "portable.json";
pub const SOURCE_PATH: &str = "metadata/source-manifest.json";
pub const EXPORT_PATH: &str = "metadata/export-validation.json";
pub const LICENCE_PATH: &str = "licence/manifest.json";
pub const CONVERTED_FROM_PATH: &str = "conversion/source-portable.json";
const V1_SCHEMA: &str = "1.0.0-dev-portable-reference-package";
pub const NAMED_REGISTRY: &str = "metadata/tables/named_registry.tsv";
pub const LINEAGE_REGISTRY: &str = "metadata/tables/lineage_registry.tsv";
pub const PRIMARY_INDEX: &str = "databases/primary_whole/index.db";

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn read_tree(root: &Path, dir: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            read_tree(root, &path, files);
        } else {
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            files.insert(relative, fs::read(&path).unwrap());
        }
    }
}

/// The fixture as a set of files, with the manifest held apart so it can be edited.
///
/// The fixture was converted from a version 1 package, so it carries that manifest. An edit that
/// re-seals the manifest re-seals that record too, as a conversion of the edited package would,
/// unless a test has edited the conversion record itself.
pub struct Synthetic {
    files: BTreeMap<String, Vec<u8>>,
    manifest: Value,
    manifest_bytes: Vec<u8>,
    conversion_follows: bool,
}

impl Synthetic {
    /// The committed fixture, exactly as it is on disk.
    pub fn new() -> Self {
        let root = Path::new(FIXTURE_DIR);
        let mut files = BTreeMap::new();
        read_tree(root, root, &mut files);
        let manifest_bytes = files.remove(MANIFEST).expect("the fixture has a manifest");
        let manifest = serde_json::from_slice(&manifest_bytes).unwrap();
        Synthetic {
            files,
            manifest,
            manifest_bytes,
            conversion_follows: true,
        }
    }

    pub fn manifest_sha256(&self) -> String {
        sha256_hex(&self.manifest_bytes)
    }

    pub fn file(&self, path: &str) -> &[u8] {
        self.files
            .get(path)
            .unwrap_or_else(|| panic!("no file {path}"))
    }

    /// Every file as it would be written to disk, with the manifest as `portable.json`.
    pub fn files(&self) -> BTreeMap<String, Vec<u8>> {
        let mut files = self.files.clone();
        files.insert(MANIFEST.into(), self.manifest_bytes.clone());
        files
    }

    pub fn source(&self) -> MemorySource {
        MemorySource::new(self.files())
    }

    /// Verify against the manifest digest the package has now, as an operator who vouches for it would.
    pub fn verify(&self) -> Result<VerifiedPackage, PackageError> {
        self.verify_with(&self.manifest_sha256(), &RegistryPins::default())
    }

    pub fn verify_with(
        &self,
        expected: &str,
        pins: &RegistryPins,
    ) -> Result<VerifiedPackage, PackageError> {
        verify_package(&self.source(), expected, pins)
    }

    pub fn write_to(&self, dir: &Path) {
        for (path, bytes) in self.files() {
            let target = dir.join(&path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, bytes).unwrap();
        }
    }

    /// Flip a byte without re-listing the file, as silent corruption would.
    pub fn alter(&mut self, path: &str) {
        self.files.get_mut(path).expect("listed file")[0] ^= 0xff;
    }

    /// Replace a file's bytes and re-list it, so only the change itself is wrong.
    pub fn rewrite(&mut self, path: &str, bytes: Vec<u8>) {
        self.files.insert(path.into(), bytes);
        self.relist(path);
    }

    pub fn truncate(&mut self, path: &str) {
        self.files.get_mut(path).expect("listed file").pop();
    }

    pub fn extend(&mut self, path: &str) {
        self.files.get_mut(path).expect("listed file").push(b'\n');
    }

    pub fn remove(&mut self, path: &str) {
        self.files.remove(path).expect("listed file");
    }

    pub fn add_unlisted(&mut self, path: &str, bytes: &[u8]) {
        self.files.insert(path.into(), bytes.to_vec());
    }

    /// Remove a file and its listing together, as an incomplete build would.
    pub fn drop_listing(&mut self, path: &str) {
        self.remove(path);
        self.manifest["assets"]
            .as_array_mut()
            .expect("assets is a list")
            .retain(|asset| asset["path"] != path);
        self.write_manifest();
    }

    /// Change a listed path in the manifest only, so the file itself is untouched.
    pub fn rename_listing(&mut self, from: &str, to: &str) {
        for asset in self.manifest["assets"]
            .as_array_mut()
            .expect("assets is a list")
        {
            if asset["path"] == from {
                asset["path"] = to.into();
            }
        }
        self.write_manifest();
    }

    /// Rewrite a file and re-list it, so the manifest agrees with the file's new length and digest.
    pub fn edit_json(&mut self, path: &str, edit: impl FnOnce(&mut Value)) {
        let mut value: Value = serde_json::from_slice(self.file(path)).unwrap();
        edit(&mut value);
        let bytes = serde_json::to_vec_pretty(&value).unwrap();
        self.files.insert(path.into(), bytes);
        self.relist(path);
    }

    /// Edit the manifest and seal it, so its digest is the one an operator would now expect.
    pub fn edit_manifest(&mut self, edit: impl FnOnce(&mut Value)) {
        edit(&mut self.manifest);
        self.write_manifest();
    }

    /// Edit the version 1 manifest this package records it was converted from, and re-list it.
    pub fn edit_conversion(&mut self, edit: impl FnOnce(&mut Value)) {
        self.conversion_follows = false;
        let mut value: Value = serde_json::from_slice(self.file(CONVERTED_FROM_PATH)).unwrap();
        edit(&mut value);
        self.files.insert(
            CONVERTED_FROM_PATH.into(),
            serde_json::to_vec_pretty(&value).unwrap(),
        );
        self.relist(CONVERTED_FROM_PATH);
    }

    /// The same package as a native version 2 export, without a conversion record.
    pub fn without_conversion(&mut self) {
        self.files.remove(CONVERTED_FROM_PATH);
        self.manifest
            .as_object_mut()
            .unwrap()
            .remove("converted_from");
        self.write_manifest();
    }

    /// Replace the manifest bytes without touching anything else, so the digest check sees them as is.
    pub fn replace_manifest_bytes(&mut self, bytes: &[u8]) {
        self.manifest_bytes = bytes.to_vec();
    }

    fn relist(&mut self, path: &str) {
        let (size, digest) = {
            let bytes = self.file(path);
            (bytes.len() as u64, sha256_hex(bytes))
        };
        let update = |record: &mut Value| {
            if record["path"] == path {
                record["bytes"] = json!(size);
                record["sha256"] = json!(digest);
            }
        };
        for key in [
            "source_manifest",
            "export_validation",
            "licence",
            "converted_from",
        ] {
            update(&mut self.manifest[key]);
        }
        for asset in self.manifest["assets"]
            .as_array_mut()
            .expect("assets is a list")
        {
            update(asset);
        }
        self.write_manifest();
    }

    fn write_manifest(&mut self) {
        if self.conversion_follows && self.manifest.get("converted_from").is_some() {
            let mut v1 = self.manifest.clone();
            let fields = v1.as_object_mut().unwrap();
            fields.remove("licence");
            fields.remove("converted_from");
            fields.insert("schema_version".into(), json!(V1_SCHEMA));
            let bytes = serde_json::to_vec_pretty(&v1).unwrap();
            self.manifest["converted_from"] = json!({
                "path": CONVERTED_FROM_PATH,
                "bytes": bytes.len(),
                "sha256": sha256_hex(&bytes),
            });
            self.files.insert(CONVERTED_FROM_PATH.into(), bytes);
        }
        self.manifest_bytes = serde_json::to_vec_pretty(&self.manifest).unwrap();
    }
}

/// Serves a fixed set of files and records which paths the verifier asked for.
pub struct MemorySource {
    files: BTreeMap<String, Vec<u8>>,
    opened: RefCell<Vec<String>>,
}

impl MemorySource {
    pub fn new(files: BTreeMap<String, Vec<u8>>) -> Self {
        MemorySource {
            files,
            opened: RefCell::new(Vec::new()),
        }
    }

    /// The paths the verifier opened, in order.
    pub fn opened(&self) -> Vec<String> {
        self.opened.borrow().clone()
    }
}

impl PackageSource for MemorySource {
    fn open(&self, path: &str) -> io::Result<Option<Box<dyn Read + '_>>> {
        self.opened.borrow_mut().push(path.to_owned());
        Ok(self
            .files
            .get(path)
            .map(|bytes| Box::new(bytes.as_slice()) as Box<dyn Read + '_>))
    }

    fn inventory(&self) -> io::Result<BTreeSet<String>> {
        Ok(self.files.keys().cloned().collect())
    }
}
