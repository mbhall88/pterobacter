//! DC-1 catalogue, trusted only as embedded bytes or through an operator-supplied SHA-256.
use super::install::{ArchivePin, FilePin, InstallPlan};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{self, Read},
    path::Path,
};

pub const SCHEMA: &str = "pterobacter.database-catalogue/1";
const EMBEDDED: &str = include_str!("catalogue.json");
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
    pub schema: String,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub fixture: Option<String>,
    pub releases: Vec<Release>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub database: String,
    pub version: String,
    pub status: String,
    #[serde(default)]
    pub default: bool,
    pub schemas: BTreeMap<String, Value>,
    pub citation: Value,
    pub components: Vec<Component>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub id: String,
    pub need: String,
    pub archive: Archive,
    pub extracted: Extracted,
    pub identity: Identity,
    pub binds: BTreeMap<String, String>,
    pub members: Vec<FilePin>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Archive {
    pub file: String,
    pub bytes: u64,
    pub sha256: String,
    pub tar_bytes: u64,
    pub urls: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Extracted {
    pub root: String,
    pub files: u64,
    pub bytes: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub manifest: String,
    pub sha256: String,
}
pub fn invalid(s: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, s.into())
}
pub fn id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.as_bytes()[0].is_ascii_alphanumeric()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}
pub fn path(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 255
        && s.split('/').all(|p| {
            p != "."
                && p != ".."
                && !p.is_empty()
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
}
pub fn https(url: &str) -> io::Result<ureq::http::Uri> {
    let uri: ureq::http::Uri = url
        .parse()
        .map_err(|_| invalid("invalid HTTPS download URL"))?;
    if uri.scheme_str() != Some("https")
        || uri.host().is_none()
        || uri.authority().is_some_and(|a| a.as_str().contains('@'))
        || url.contains('#')
    {
        return Err(invalid(
            "downloads require HTTPS URLs without credentials or fragments",
        ));
    }
    Ok(uri)
}
impl Component {
    pub fn pin(&self) -> ArchivePin {
        ArchivePin {
            component: self.id.clone(),
            archive_root: Some(self.extracted.root.clone()),
            bytes: self.archive.bytes,
            sha256: self.archive.sha256.clone(),
            tar_bytes_limit: self.archive.tar_bytes,
            files: self.members.clone(),
        }
    }
}
impl Catalogue {
    pub fn load(file: Option<&Path>, expected: Option<&str>) -> io::Result<(Self, String)> {
        let bytes = match (file, expected) {
            (None, None) => EMBEDDED.as_bytes().to_vec(),
            (Some(p), Some(pin)) => {
                if !crate::reference::is_sha256(pin) {
                    return Err(invalid("malformed catalogue SHA-256"));
                }
                let mut bytes = Vec::new();
                File::open(p)?
                    .take((16 << 20) + 1)
                    .read_to_end(&mut bytes)?;
                if bytes.len() > 16 << 20 || crate::evidence::sha256_hex(&bytes) != pin {
                    return Err(invalid("catalogue size or SHA-256 mismatch"));
                }
                bytes
            }
            _ => {
                return Err(invalid(
                    "--catalogue and --catalogue-sha256 must be supplied together",
                ))
            }
        };
        let value: Self =
            serde_json::from_slice(&bytes).map_err(|e| invalid(format!("catalogue: {e}")))?;
        value.validate()?;
        Ok((value, crate::evidence::sha256_hex(&bytes)))
    }
    pub fn validate(&self) -> io::Result<()> {
        if self.schema != SCHEMA {
            return Err(invalid("unsupported catalogue schema"));
        }
        let mut versions = BTreeSet::new();
        let mut defaults = 0;
        for r in &self.releases {
            if !id(&r.database)
                || !id(&r.version)
                || !versions.insert((&r.database, &r.version))
                || !["candidate", "published", "withdrawn"].contains(&r.status.as_str())
            {
                return Err(invalid("invalid or duplicate catalogue release"));
            }
            if r.default {
                defaults += 1;
                if r.status != "published" {
                    return Err(invalid("only a published release may be a default"));
                }
            }
            let plan = InstallPlan {
                version: r.version.clone(),
                archives: r.components.iter().map(Component::pin).collect(),
            };
            plan.validate()?;
            let components: BTreeMap<_, _> =
                r.components.iter().map(|c| (c.id.as_str(), c)).collect();
            let mut archive_names = BTreeSet::new();
            if !components.contains_key("reference") {
                return Err(invalid("release has no reference component"));
            }
            for c in &r.components {
                if ![
                    "reference",
                    "primary-fasta",
                    "mlst-historical",
                    "mlst-custom",
                ]
                .contains(&c.id.as_str())
                    || c.extracted.root != c.id
                    || !path(&c.archive.file)
                    || c.archive.file.contains('/')
                    || !archive_names.insert(&c.archive.file)
                    || c.archive.tar_bytes % 512 != 0
                    || c.extracted.files != c.members.len() as u64
                {
                    return Err(invalid(
                        "invalid component identity, root or inventory count",
                    ));
                }
                let size = c.members.iter().try_fold(0u64, |n, m| {
                    n.checked_add(m.bytes)
                        .ok_or_else(|| invalid("inventory size overflow"))
                })?;
                if size != c.extracted.bytes
                    || !path(&c.identity.manifest)
                    || !crate::reference::is_sha256(&c.identity.sha256)
                    || c.members.iter().any(|m| !path(&m.path))
                    || !c
                        .members
                        .iter()
                        .any(|m| m.path == c.identity.manifest && m.sha256 == c.identity.sha256)
                {
                    return Err(invalid("catalogue inventory/manifest mismatch"));
                }
                let (need, manifest, schema) = match c.id.as_str() {
                    "reference" => (
                        "required",
                        "portable.json",
                        r.schemas.get("reference").is_some_and(|v| {
                            v == "2.0.0-dev-portable-reference-package"
                                || v == "2.1.0-public-portable-reference-package"
                        }),
                    ),
                    "primary-fasta" => (
                        "default_workflow",
                        "anim.json",
                        r.schemas
                            .get("primary-fasta")
                            .is_some_and(|v| v == "terobacter.anim-reference/1"),
                    ),
                    "mlst-historical" => (
                        "optional",
                        "manifest.json",
                        r.schemas.get("mlst-historical") == Some(&serde_json::json!(1)),
                    ),
                    _ => (
                        "optional",
                        "manifest.json",
                        r.schemas.get("mlst-custom") == Some(&serde_json::json!(2)),
                    ),
                };
                if c.need != need || c.identity.manifest != manifest || !schema {
                    return Err(invalid("incompatible component schema or role"));
                }
                if c.id == "primary-fasta" {
                    if c.binds.len() != 1
                        || c.binds.get("reference")
                            != Some(&components["reference"].identity.sha256)
                    {
                        return Err(invalid("catalogue ANIm/reference binding mismatch"));
                    }
                } else if !c.binds.is_empty() {
                    return Err(invalid("unexpected component binding"));
                }
                if r.status == "candidate" && !c.archive.urls.is_empty() {
                    return Err(invalid(
                        "candidate releases must use local archives without URLs",
                    ));
                }
                for url in &c.archive.urls {
                    https(url)?;
                }
            }
            if r.schemas.len() != r.components.len() {
                return Err(invalid("schema inventory differs from components"));
            }
        }
        if defaults > 1 {
            return Err(invalid(
                "catalogue has multiple defaults; select an exact release",
            ));
        }
        Ok(())
    }
    pub fn select(&self, version: Option<&str>) -> io::Result<&Release> {
        let found: Vec<_> = self
            .releases
            .iter()
            .filter(|r| version.map_or(r.default, |v| r.version == v))
            .collect();
        if found.len() != 1 {
            return Err(invalid(match version { Some(v)=>format!("exact database version {v:?} is missing or ambiguous in the trusted catalogue"), None=>"no published default database is pinned; select --database VERSION from a trusted catalogue or use --reference-package and --expected-sha256".into() }));
        }
        let r = found[0];
        if r.status == "withdrawn" {
            eprintln!(
                "warning: explicitly selected withdrawn database {} {}",
                r.database, r.version
            );
        }
        Ok(r)
    }
}
impl Release {
    pub fn component(&self, id: &str) -> io::Result<&Component> {
        self.components
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| invalid(format!("database {} lacks component {id}", self.version)))
    }
    pub fn typing(&self) -> Option<&Component> {
        self.components
            .iter()
            .find(|c| c.id == "mlst-custom")
            .or_else(|| self.components.iter().find(|c| c.id == "mlst-historical"))
    }
}
