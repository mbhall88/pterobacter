//! A synthetic reference package with the layout of the portable package. It is built in memory
//! so a test can damage exactly one thing and check the rejection that follows.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use terobacter::reference::PackageSource;

pub const PACKAGE_VERSION: &str = "2026-10-09.test1";
pub const DISCLAIMER: &str = "Synthetic test disclaimer.";
pub const MANIFEST: &str = "portable.json";
pub const LICENCE_PATH: &str = "licence/manifest.json";
pub const EXPORT_PATH: &str = "metadata/export-validation.json";
const SOURCE_PATH: &str = "metadata/source-manifest.json";
const PACKAGE_SCHEMA: &str = "1.0.0-dev-portable-reference-package";
const LICENCE_SCHEMA: &str = "1.0.0-reference-licence-manifest";
const COMPONENTS: [&str; 2] = ["primary_whole", "genomovar_chromosome"];
const DATABASE_FILES: [&str; 3] = ["index.db", "markers.bin", "sketches.db"];

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn licence(package_version: &str, disclaimer: &str) -> Value {
    json!({
        "schema_version": LICENCE_SCHEMA,
        "package_version": package_version,
        "ncbi_submitter_rights_disclaimer": disclaimer,
    })
}

fn record(bytes: &[u8], path: &str, roles: Option<Vec<String>>) -> Value {
    let mut record = json!({"path": path, "bytes": bytes.len(), "sha256": sha256_hex(bytes)});
    if let Some(roles) = roles {
        record["roles"] = json!(roles);
    }
    record
}

pub struct Synthetic {
    files: BTreeMap<String, Vec<u8>>,
    manifest: Value,
    raw_manifest: Option<Vec<u8>>,
}

impl Synthetic {
    pub fn new() -> Self {
        let mut files = BTreeMap::new();
        let mut assets = Vec::new();
        let mut put = |path: String, roles: Vec<String>, bytes: Vec<u8>| {
            assets.push(record(&bytes, &path, Some(roles)));
            files.insert(path, bytes);
        };
        for component in COMPONENTS {
            for file in DATABASE_FILES {
                put(
                    format!("databases/{component}/{file}"),
                    vec![format!("database:{component}:{file}")],
                    format!("{component} {file}").into_bytes(),
                );
            }
            put(
                format!("build/{component}/build.json"),
                vec![format!("build:{component}:0")],
                format!("{{\"component\":\"{component}\"}}").into_bytes(),
            );
        }
        put(
            "tables/named_registry.tsv".into(),
            vec!["table:named_registry".into()],
            b"concept_id\tname\n".to_vec(),
        );
        put(
            LICENCE_PATH.into(),
            vec!["licence:manifest".into()],
            licence(PACKAGE_VERSION, DISCLAIMER)
                .to_string()
                .into_bytes(),
        );

        let export = b"{}".to_vec();
        let source = b"{}".to_vec();
        let export_record = record(&export, EXPORT_PATH, None);
        let source_record = record(&source, SOURCE_PATH, None);
        files.insert(EXPORT_PATH.into(), export);
        files.insert(SOURCE_PATH.into(), source);

        let manifest = json!({
            "schema_version": PACKAGE_SCHEMA,
            "package_id": "terobacter-reference-test",
            "package_version": PACKAGE_VERSION,
            "release_state": "synthetic_test_fixture",
            "trust_basis": "synthetic_test_fixture",
            "positive_calling_policy_enabled": false,
            "assets": assets,
            "export_validation": export_record,
            "source_manifest": source_record,
        });
        Synthetic {
            files,
            manifest,
            raw_manifest: None,
        }
    }

    /// Every file as it would be written to disk, with the manifest as `portable.json`.
    pub fn files(&self) -> BTreeMap<String, Vec<u8>> {
        let mut files = self.files.clone();
        files.insert(MANIFEST.into(), self.manifest_bytes());
        files
    }

    pub fn manifest_sha256(&self) -> String {
        sha256_hex(&self.manifest_bytes())
    }

    pub fn source(&self) -> MemorySource {
        MemorySource(self.files())
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

    pub fn truncate(&mut self, path: &str) {
        self.files.get_mut(path).expect("listed file").pop();
    }

    pub fn remove(&mut self, path: &str) {
        self.files.remove(path).expect("listed file");
    }

    /// Remove a file and its listing together, as an incomplete build would.
    pub fn drop_listing(&mut self, path: &str) {
        self.remove(path);
        self.manifest["assets"]
            .as_array_mut()
            .unwrap()
            .retain(|asset| asset["path"] != path);
    }

    /// Replace the licence manifest and re-list it with its new digest, as a rebuild would.
    pub fn replace_licence(&mut self, body: Value) {
        let bytes = body.to_string().into_bytes();
        let entry = record(&bytes, LICENCE_PATH, Some(vec!["licence:manifest".into()]));
        self.files.insert(LICENCE_PATH.into(), bytes);
        let assets = self.manifest["assets"].as_array_mut().unwrap();
        let slot = assets
            .iter_mut()
            .find(|asset| asset["path"] == LICENCE_PATH)
            .unwrap();
        *slot = entry;
    }

    pub fn set_manifest(&mut self, key: &str, value: Value) {
        self.manifest[key] = value;
    }

    /// Change a listed path in the manifest only, so the file itself is untouched.
    pub fn rename_listing(&mut self, from: &str, to: &str) {
        for asset in self.manifest["assets"].as_array_mut().unwrap() {
            if asset["path"] == from {
                asset["path"] = to.into();
            }
        }
    }

    pub fn replace_manifest_bytes(&mut self, bytes: &[u8]) {
        self.raw_manifest = Some(bytes.to_vec());
    }

    fn manifest_bytes(&self) -> Vec<u8> {
        self.raw_manifest
            .clone()
            .unwrap_or_else(|| serde_json::to_vec_pretty(&self.manifest).unwrap())
    }
}

pub struct MemorySource(pub BTreeMap<String, Vec<u8>>);

impl PackageSource for MemorySource {
    fn open(&self, path: &str) -> io::Result<Option<Box<dyn Read + '_>>> {
        Ok(self
            .0
            .get(path)
            .map(|bytes| Box::new(bytes.as_slice()) as Box<dyn Read + '_>))
    }
}
