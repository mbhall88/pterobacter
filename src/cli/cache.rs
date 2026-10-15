//! Reuse of completed reciprocal ANIm pairs across runs.
//!
//! An entry is reused only when its saved key equals the requested comparison in every
//! field: query and reference bytes, identities, component, scope, method and native build,
//! and the settings that can change whether a pair completes. The saved pair is then verified
//! again as [`verify_pair`] verifies any saved record. Entries are written once by renaming a
//! complete staging directory into place; an existing entry, valid or not, is never replaced.
use crate::anim::{
    hash_bytes,
    limits::{Execution, Restrictions, EXECUTION_PROFILE},
    verify_pair, ExpectedPair, Method, PairRecord, PairRequest,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub const KEY_SCHEMA: &str = "terobacter.anim-cache-key/2";
/// Entries live under this directory of the cache root, so a later key schema can sit beside it.
const ENTRIES: &str = "anim-pairs-2";

/// Everything that must match for a saved pair to stand for a new measurement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheKey {
    pub schema: String,
    pub query: QueryKey,
    pub reference: ReferenceKey,
    /// The native method and build, without the executable hash shared by the aliases.
    pub method: Method,
    pub settings: SettingsKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryKey {
    /// SHA-256 of the exact supplied bytes, which is also the query's identity in the record.
    pub sha256: String,
    pub component: String,
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceKey {
    pub reference_id: String,
    pub sha256: String,
    pub component: String,
    pub concept_ids: Vec<String>,
    pub scope: String,
    /// The authenticated primary package the reference component is bound to.
    pub package_sha256: String,
}

/// Limits that decide whether a pair completes. A pair that finished under a looser limit
/// could have failed under the requested one, so a different limit is a different comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsKey {
    pub execution_profile: String,
    #[serde(deserialize_with = "crate::anim::limits::required_option")]
    pub pair_timeout: Option<Duration>,
    #[serde(deserialize_with = "crate::anim::limits::required_option")]
    pub memory_mib: Option<u64>,
    pub inherited: Restrictions,
}

impl CacheKey {
    pub fn new(query_sha256: &str, reference: ReferenceKey, execution: &Execution) -> Self {
        Self {
            schema: KEY_SCHEMA.into(),
            query: QueryKey {
                sha256: query_sha256.into(),
                component: "query-whole-assembly".into(),
                scope: "whole_assembly_as_supplied".into(),
            },
            reference,
            method: Method::identity(),
            settings: SettingsKey {
                execution_profile: execution.profile.clone(),
                pair_timeout: execution.configured_pair_timeout,
                memory_mib: execution.memory_mib,
                inherited: execution.inherited.clone(),
            },
        }
    }

    /// The identities a saved record must carry, taken from this key and never from the record.
    pub fn expected_pair(&self) -> ExpectedPair {
        ExpectedPair {
            query_id: self.query.sha256.clone(),
            query_sha256: self.query.sha256.clone(),
            reference_id: self.reference.reference_id.clone(),
            reference_sha256: self.reference.sha256.clone(),
            reference_component: self.reference.component.clone(),
            reference_concept_ids: self.reference.concept_ids.clone(),
        }
    }

    pub fn sha256(&self) -> String {
        hash_bytes(&serde_json::to_vec(self).expect("key serializes"))
    }
}

/// What a lookup found.
#[derive(Debug)]
pub enum Lookup {
    Miss,
    /// The verified saved pair directory.
    Hit(PathBuf),
    /// An entry exists at this key's address but cannot stand for it.
    Rejected(String),
}

/// What a store did.
#[derive(Debug, PartialEq, Eq)]
pub enum Stored {
    Stored,
    /// Another entry was already in place, for example from a concurrent query. It is kept.
    Existing,
}

pub struct AnimCache {
    entries: PathBuf,
}

static STAGING: AtomicU64 = AtomicU64::new(0);

impl AnimCache {
    /// Open a cache directory, creating it when absent.
    pub fn open(directory: &Path) -> Result<Self, String> {
        let entries = directory.join(ENTRIES);
        fs::create_dir_all(&entries).map_err(|e| format!("ANIm cache: {e}"))?;
        Ok(Self { entries })
    }

    pub fn entry(&self, key: &CacheKey) -> PathBuf {
        self.entries.join(key.sha256())
    }

    pub fn lookup(&self, key: &CacheKey) -> Lookup {
        let entry = self.entry(key);
        match fs::symlink_metadata(&entry) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Lookup::Miss,
            Err(error) => return Lookup::Rejected(error.to_string()),
            Ok(metadata) if !metadata.is_dir() => {
                return Lookup::Rejected("entry is not a directory".into())
            }
            Ok(_) => {}
        }
        let saved = fs::read(entry.join("key.json"))
            .map_err(|e| e.to_string())
            .and_then(|bytes| {
                serde_json::from_slice::<CacheKey>(&bytes).map_err(|e| e.to_string())
            });
        match saved {
            Err(error) => return Lookup::Rejected(format!("unreadable entry key: {error}")),
            Ok(saved) if saved != *key => {
                return Lookup::Rejected("entry key differs from the requested comparison".into())
            }
            Ok(_) => {}
        }
        let pair = entry.join("pair");
        match verify_pair(&pair, &key.expected_pair()).map_err(|e| e.to_string()) {
            Ok(record) => match record_matches(&pair, &record, key) {
                Ok(()) => Lookup::Hit(pair),
                Err(why) => Lookup::Rejected(why),
            },
            Err(error) => Lookup::Rejected(error),
        }
    }

    /// Save a verified complete pair under `key`. The entry appears whole or not at all.
    pub fn store(&self, key: &CacheKey, pair: &Path) -> Result<Stored, String> {
        let entry = self.entry(key);
        if fs::symlink_metadata(&entry).is_ok() {
            return Ok(Stored::Existing);
        }
        let staging = self.entries.join(format!(
            ".staging-{}-{}-{}",
            key.sha256(),
            std::process::id(),
            STAGING.fetch_add(1, Ordering::SeqCst)
        ));
        let result = (|| -> Result<Stored, String> {
            fs::create_dir(&staging).map_err(|e| e.to_string())?;
            copy_tree(pair, &staging.join("pair")).map_err(|e| e.to_string())?;
            let record = verify_pair(&staging.join("pair"), &key.expected_pair())
                .map_err(|e| e.to_string())?;
            record_matches(&staging.join("pair"), &record, key)?;
            let mut text = serde_json::to_vec_pretty(key).map_err(|e| e.to_string())?;
            text.push(b'\n');
            fs::write(staging.join("key.json"), text).map_err(|e| e.to_string())?;
            // Renaming a directory never replaces a non-empty one, so a concurrent store
            // that finished first keeps its entry.
            match fs::rename(&staging, &entry) {
                Ok(()) => Ok(Stored::Stored),
                Err(_) if fs::symlink_metadata(entry.join("key.json")).is_ok() => {
                    Ok(Stored::Existing)
                }
                Err(error) => Err(error.to_string()),
            }
        })();
        if fs::symlink_metadata(&staging).is_ok() {
            let _ = fs::remove_dir_all(&staging);
        }
        result
    }
}

/// The saved record's own method and limits agree with its key, so a key cannot describe
/// a comparison other than the one the record holds.
fn record_matches(pair: &Path, record: &PairRecord, key: &CacheKey) -> Result<(), String> {
    let method = Method {
        implementation_sha256: String::new(),
        ..record.method.clone()
    };
    if method != key.method {
        return Err("saved method differs from the entry key".into());
    }
    let request: PairRequest = fs::read(pair.join("request.json"))
        .map_err(|e| e.to_string())
        .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|e| e.to_string()))?;
    let execution = request
        .execution
        .as_ref()
        .ok_or("historical execution cannot enter the new cache")?;
    execution.validate().map_err(|e| e.to_string())?;
    if key.schema != KEY_SCHEMA
        || key.settings.execution_profile != EXECUTION_PROFILE
        || execution.profile != key.settings.execution_profile
        || execution.configured_pair_timeout != key.settings.pair_timeout
        || execution.memory_mib != key.settings.memory_mib
        || execution.inherited != key.settings.inherited
    {
        return Err("saved limits differ from the entry key".into());
    }
    Ok(())
}

/// Copy a directory of regular files and directories to a new location.
pub fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &target)?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "saved pairs hold only regular files and directories",
            ));
        }
    }
    Ok(())
}
