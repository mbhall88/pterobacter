//! Version-serialized, immutable component activation. Receipts never authorize reuse.
use super::{
    catalogue::{invalid, Component, Release},
    install,
};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn ensure_tree(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => Ok(()),
        Ok(m) if m.is_symlink() && path.is_dir() => Ok(()), // platform ancestors may be links; store itself is checked separately
        Ok(_) => Err(invalid("application path is not a directory")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                ensure_tree(parent)?;
            }
            match install::private_dir(path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => ensure_tree(path),
                Err(e) => Err(e),
            }
        }
        Err(e) => Err(e),
    }
}
struct Lock(PathBuf);
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
struct Stage(PathBuf);
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
pub struct Store {
    pub version: PathBuf,
    _lock: Lock,
}
#[derive(Debug)]
pub struct ActivationError {
    pub activated: bool,
    pub source: io::Error,
}
impl std::fmt::Display for ActivationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.activated {
            write!(f,"component was activated, but durability or receipt recording failed; retry will fully verify existing bytes: {}",self.source)
        } else {
            write!(f, "{}", self.source)
        }
    }
}
impl std::error::Error for ActivationError {}
impl From<io::Error> for ActivationError {
    fn from(source: io::Error) -> Self {
        Self {
            activated: false,
            source,
        }
    }
}
impl Store {
    pub fn open(root: &Path, release: &Release) -> io::Result<Self> {
        ensure_tree(root)?;
        let root = install::private_root(root)?;
        let db = root.join(&release.database);
        ensure_tree(&db)?;
        let db = install::private_root(&db)?;
        let lock_path = db.join(format!(".{}.lock", release.version));
        let mut file=install::private_file(&lock_path).map_err(|e|io::Error::new(e.kind(),format!("database version is locked or unavailable; inspect its owner before cleanup: {e}")))?;
        let lock = Lock(lock_path);
        writeln!(file, "pid={}", std::process::id())?;
        file.sync_all()?;
        let version = db.join(&release.version);
        ensure_tree(&version)?;
        let version = install::private_root(&version)?;
        Ok(Self {
            version,
            _lock: lock,
        })
    }
    pub fn install(
        &self,
        component: &Component,
        catalogue_sha: &str,
        fetch: impl FnOnce() -> io::Result<Box<dyn Read>>,
        verify: impl Fn(&Path) -> io::Result<()>,
    ) -> Result<&'static str, ActivationError> {
        self.install_with(
            component,
            fetch,
            verify,
            |dir| File::open(dir)?.sync_all(),
            |path| self.receipt(component, catalogue_sha, path),
        )
    }
    fn install_with(
        &self,
        component: &Component,
        fetch: impl FnOnce() -> io::Result<Box<dyn Read>>,
        verify: impl Fn(&Path) -> io::Result<()>,
        durable: impl FnOnce(&Path) -> io::Result<()>,
        finish: impl FnOnce(&Path) -> io::Result<()>,
    ) -> Result<&'static str, ActivationError> {
        let pin = component.pin();
        let target = self.version.join(&component.id);
        match fs::symlink_metadata(&target) {
            Ok(_) => {
                install::verify_component(&target, &pin)?;
                verify(&target)?;
                install::verify_component(&target, &pin)?;
                finish(&target).map_err(|source| ActivationError {
                    activated: true,
                    source,
                })?;
                return Ok("verified_reuse");
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        let required = component
            .extracted
            .bytes
            .checked_add(component.archive.bytes)
            .and_then(|n| n.checked_add(1 << 20))
            .ok_or_else(|| invalid("component size overflow"))?;
        if install::available_space(&self.version)? < required {
            return Err(io::Error::other("insufficient available space for component").into());
        }
        let stage = install::new_stage(&self.version, &component.id)?;
        let _guard = Stage(stage.clone());
        let download = stage.join("archive.partial");
        let mut file = install::private_file(&download)?;
        install::copy_pinned(&mut fetch()?, &mut file, pin.bytes, &pin.sha256)?;
        file.sync_all()?;
        drop(file);
        let extracted = stage.join(&component.id);
        install::private_dir(&extracted)?;
        install::extract_checked(&download, &extracted, &pin, true)?;
        install::verify_component(&extracted, &pin)?;
        verify(&extracted)?;
        install::verify_component(&extracted, &pin)?;
        install::sync_directories(&extracted)?;
        install::rename_no_replace(&extracted,&target).map_err(|e|io::Error::new(e.kind(),format!("{e}; choose --database-dir on supported local storage or use explicit package paths")))?;
        // After this point, failures retain the activated component and advertise that distinction.
        durable(&self.version)
            .and_then(|()| finish(&target))
            .map_err(|source| ActivationError {
                activated: true,
                source,
            })?;
        Ok("installed")
    }
    fn receipt(&self, component: &Component, sha: &str, _target: &Path) -> io::Result<()> {
        let directory = self.version.join("receipts");
        ensure_tree(&directory)?;
        install::private_root(&directory)?;
        let receipt = directory.join(format!("{}.json", component.id));
        if fs::symlink_metadata(&receipt).is_ok() {
            return Ok(());
        }
        let bytes = serde_json::to_vec_pretty(
            &serde_json::json!({"schema":"pterobacter.install-receipt/1","catalogue_sha256":sha,"component":component.id,"archive_sha256":component.archive.sha256,"manifest_sha256":component.identity.sha256,"tool_version":env!("CARGO_PKG_VERSION"),"verified_unix_seconds":SystemTime::now().duration_since(UNIX_EPOCH).map_err(io::Error::other)?.as_secs()}),
        )?;
        super::super::outputs::write_new(&receipt, &bytes)?;
        File::open(directory)?.sync_all()
    }
}

#[cfg(test)]
mod tests;
