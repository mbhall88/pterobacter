//! Pinned archive staging, independent of catalogue serialization and scientific package schemas.
use flate2::bufread::GzDecoder;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_ARCHIVE_BYTES: u64 = 16 << 30;
const MAX_EXPANDED_BYTES: u64 = 64 << 30;
const MAX_FILES: usize = 100_000;
static NEXT: AtomicU64 = AtomicU64::new(0);

/// A regular file's expected transport-relative path and exact bytes.
#[derive(Clone, Debug)]
pub struct FilePin {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

/// A gzip-compressed portable tar archive, pinned outside the archive itself.
#[derive(Clone, Debug)]
pub struct ArchivePin {
    /// Also the component directory name beneath the installed version.
    pub component: String,
    /// Exact optional top-level directory in the tar, stripped after validation.
    pub archive_root: Option<String>,
    pub bytes: u64,
    pub sha256: String,
    /// Maximum decoded tar bytes, including headers/padding; a decompression bound.
    pub tar_bytes_limit: u64,
    /// Exhaustive regular-file inventory. Directory entries are optional.
    pub files: Vec<FilePin>,
}

/// Transport inputs from a trusted adapter, never deserialized from an archive.
#[derive(Clone, Debug)]
pub struct InstallPlan {
    pub version: String,
    pub archives: Vec<ArchivePin>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Installed {
    New(PathBuf),
    Reused(PathBuf),
}
impl Installed {
    pub fn path(&self) -> &Path {
        match self {
            Self::New(path) | Self::Reused(path) => path,
        }
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        && value != "."
        && value != ".."
}
fn safe_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && !value.contains(['\\', ':', '\0'])
        && value.split('/').all(|part| {
            !part.is_empty() && part != "." && part != ".." && !part.chars().any(char::is_control)
        })
}
fn add(sum: u64, n: u64) -> io::Result<u64> {
    sum.checked_add(n).ok_or_else(|| invalid("size overflow"))
}
impl InstallPlan {
    /// Validate pins and compute a conservative disk requirement before fetching anything.
    pub fn validate(&self) -> io::Result<u64> {
        if !identifier(&self.version) || self.archives.is_empty() || self.archives.len() > 16 {
            return Err(invalid("invalid version or component count"));
        }
        let mut components = BTreeSet::new();
        let mut required = 0;
        for archive in &self.archives {
            if !identifier(&archive.component)
                || archive
                    .archive_root
                    .as_deref()
                    .is_some_and(|r| !identifier(r))
                || !components.insert(&archive.component)
            {
                return Err(invalid("invalid or duplicate component"));
            }
            if archive.bytes == 0
                || archive.bytes > MAX_ARCHIVE_BYTES
                || !crate::reference::is_sha256(&archive.sha256)
                || archive.tar_bytes_limit < 1024
                || archive.tar_bytes_limit > MAX_EXPANDED_BYTES
                || archive.files.is_empty()
                || archive.files.len() > MAX_FILES
            {
                return Err(invalid(
                    "invalid archive size, digest, decoded limit or inventory",
                ));
            }
            required = add(required, archive.bytes)?;
            let mut files = BTreeSet::new();
            let mut expanded = 0;
            for file in &archive.files {
                if !safe_path(&file.path)
                    || !crate::reference::is_sha256(&file.sha256)
                    || !files.insert(file.path.as_str())
                {
                    return Err(invalid("invalid or duplicate file pin"));
                }
                expanded = add(expanded, file.bytes)?;
                required = add(required, file.bytes)?;
            }
            if expanded > archive.tar_bytes_limit {
                return Err(invalid("file inventory exceeds decoded archive limit"));
            }
            // No file may double as an ancestor directory of another file.
            for path in &files {
                let mut parent = Path::new(path).parent();
                while let Some(p) = parent.filter(|p| !p.as_os_str().is_empty()) {
                    if files.contains(p.to_str().expect("validated UTF-8")) {
                        return Err(invalid("file/directory collision in inventory"));
                    }
                    parent = p.parent();
                }
            }
        }
        Ok(required)
    }
}

/// Fetch and install all components. `fetch` must return the requested pinned byte stream;
/// `verify_science` must authenticate scientific manifests and all cross-component bindings.
/// It runs after extraction or before reuse; transport bytes are rechecked afterwards. No network operation occurs on reuse.
///
/// The store must be private to its Unix owner. Interrupted locks/stages are preserved for explicit
/// inspection; this code never guesses that another process's lock is stale.
pub fn install(
    root: &Path,
    plan: &InstallPlan,
    fetch: impl FnMut(&ArchivePin) -> io::Result<Box<dyn Read>>,
    verify_science: impl Fn(&Path) -> io::Result<()>,
) -> io::Result<Installed> {
    install_with_space(root, plan, fetch, verify_science, available_space)
}

fn install_with_space(
    root: &Path,
    plan: &InstallPlan,
    mut fetch: impl FnMut(&ArchivePin) -> io::Result<Box<dyn Read>>,
    verify_science: impl Fn(&Path) -> io::Result<()>,
    space: impl FnOnce(&Path) -> io::Result<u64>,
) -> io::Result<Installed> {
    let required = plan.validate()?;
    let root = private_root(root)?;
    let lock_path = root.join(format!(".{}.lock", plan.version));
    let mut lock = private_file(&lock_path)?;
    let _lock_guard = RemoveFile(lock_path);
    writeln!(lock, "pid={}", std::process::id())?;
    lock.sync_all()?;
    let target = root.join(&plan.version);
    match fs::symlink_metadata(&target) {
        Ok(_) => {
            verify(&target, plan, &verify_science)?;
            return Ok(Installed::Reused(target));
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    if space(&root)? < required {
        return Err(io::Error::other(
            "insufficient available space for pinned components",
        ));
    }
    let stage = new_stage(&root, &plan.version)?;
    let _stage_guard = RemoveTree(stage.clone());
    for archive in &plan.archives {
        let archive_path = stage.join(format!(".{}.download", archive.component));
        let mut output = private_file(&archive_path)?;
        copy_pinned(
            &mut fetch(archive)?,
            &mut output,
            archive.bytes,
            &archive.sha256,
        )?;
        output.sync_all()?;
        drop(output);
        let component = stage.join(&archive.component);
        private_dir(&component)?;
        extract(&archive_path, &component, archive)?;
        fs::remove_file(archive_path)?;
    }
    verify(&stage, plan, &verify_science)?;
    // Recheck after the scientific verifier: it must not introduce or mutate transport files.
    verify_files(&stage, plan)?;
    sync_directories(&stage)?;
    rename_no_replace(&stage, &target)?;
    File::open(&root)?.sync_all()?;
    Ok(Installed::New(target))
}

/// Verify exact inventory and hashes, then invoke the scientific component verifier.
/// The caller supplies the same trusted plan used at installation; a local receipt is not authority.
pub fn verify(
    directory: &Path,
    plan: &InstallPlan,
    verify_science: impl Fn(&Path) -> io::Result<()>,
) -> io::Result<()> {
    plan.validate()?;
    verify_files(directory, plan)?;
    verify_science(directory)?;
    verify_files(directory, plan)
}

fn private_dir(path: &Path) -> io::Result<()> {
    fs::DirBuilder::new().mode(0o700).create(path)
}
fn private_file(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}
fn private_root(path: &Path) -> io::Result<PathBuf> {
    match private_dir(path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o022 != 0
    {
        return Err(invalid(format!("database store must be an owned directory without group/world write access: uid={}, euid={}, mode={:o}, directory={}", metadata.uid(), unsafe { libc::geteuid() }, metadata.mode(), metadata.is_dir())));
    }
    fs::canonicalize(path)
}
fn new_stage(root: &Path, version: &str) -> io::Result<PathBuf> {
    new_directory(root, &format!(".{version}.staging"))
}
fn new_directory(root: &Path, prefix: &str) -> io::Result<PathBuf> {
    for _ in 0..128 {
        let stage = root.join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match private_dir(&stage) {
            Ok(()) => return Ok(stage),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::other(
        "could not reserve an unused staging directory",
    ))
}
pub(super) fn reserve_evidence(root: &Path) -> io::Result<PathBuf> {
    let root = private_root(root)?;
    let path = new_directory(&root, "evidence-run")?;
    File::open(&root)?.sync_all()?;
    Ok(path)
}

struct RemoveFile(PathBuf);
impl Drop for RemoveFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
struct RemoveTree(PathBuf);
impl Drop for RemoveTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_pinned(
    reader: &mut dyn Read,
    writer: &mut dyn Write,
    bytes: u64,
    sha: &str,
) -> io::Result<()> {
    let mut digest = Sha256::new();
    let mut seen = 0;
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        seen = add(seen, count as u64)?;
        if seen > bytes {
            return Err(invalid("stream exceeds pinned byte size"));
        }
        digest.update(&buffer[..count]);
        writer.write_all(&buffer[..count])?;
    }
    if seen != bytes || format!("{:x}", digest.finalize()) != sha {
        return Err(invalid("stream byte size or SHA-256 mismatch"));
    }
    Ok(())
}

/// A decoding ceiling applies even to archive metadata, sparse entries and trailing padding.
struct Bounded<R> {
    reader: R,
    remaining: u64,
    seen: u64,
}
impl<R: Read> Read for Bounded<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let limit = buffer.len().min(self.remaining.saturating_add(1) as usize);
        let n = self.reader.read(&mut buffer[..limit])?;
        if n as u64 > self.remaining {
            return Err(invalid("decoded tar exceeds pinned ceiling"));
        }
        self.remaining -= n as u64;
        self.seen += n as u64;
        Ok(n)
    }
}
fn extract(archive: &Path, destination: &Path, pin: &ArchivePin) -> io::Result<()> {
    let reader = Bounded {
        reader: GzDecoder::new(BufReader::new(File::open(archive)?)),
        remaining: pin.tar_bytes_limit,
        seen: 0,
    };
    let mut archive = tar::Archive::new(reader);
    let expected: BTreeMap<_, _> = pin.files.iter().map(|p| (p.path.as_str(), p)).collect();
    let directories = parents(expected.keys().copied());
    let mut seen = BTreeSet::new();
    let mut found = BTreeSet::new();
    let mut last_end = 0;
    // Raw mode refuses long-name/PAX/sparse metadata instead of reading attacker-sized metadata
    // into memory. Exports must use ordinary portable tar paths, files and directories.
    for entry in archive.entries()?.raw(true) {
        let mut entry = entry?;
        let kind = entry.header().entry_type();
        if entry.header().as_bytes().contains(&b'\\') {
            return Err(invalid("backslashes in archive headers are forbidden"));
        }
        last_end = add(
            entry.raw_header_position(),
            add(
                512,
                entry
                    .size()
                    .div_ceil(512)
                    .checked_mul(512)
                    .ok_or_else(|| invalid("tar size overflow"))?,
            )?,
        )?;
        if !kind.is_file() && !kind.is_dir() {
            return Err(invalid(
                "archive links, metadata and special members are forbidden",
            ));
        }
        if entry.header().link_name_bytes().is_some() {
            return Err(invalid("archive member has a link target"));
        }
        let raw = entry.path_bytes().into_owned();
        let path = std::str::from_utf8(&raw).map_err(|_| invalid("archive path is not UTF-8"))?;
        let path = if kind.is_dir() {
            path.strip_suffix('/').unwrap_or(path)
        } else {
            path
        };
        if !safe_path(path) || !seen.insert(path.to_owned()) {
            return Err(invalid("unsafe or duplicate archive path"));
        }
        let path = if let Some(root) = &pin.archive_root {
            if path == root && kind.is_dir() && entry.size() == 0 {
                continue;
            }
            path.strip_prefix(root.as_str())
                .and_then(|p| p.strip_prefix('/'))
                .ok_or_else(|| invalid("archive member is outside the pinned root"))?
        } else {
            path
        };
        if kind.is_dir() {
            if entry.size() != 0 || !directories.contains(path) {
                return Err(invalid("unexpected archive directory"));
            }
            continue;
        }
        let file = expected
            .get(path)
            .ok_or_else(|| invalid("unexpected archive file"))?;
        if entry.size() != file.bytes {
            return Err(invalid("archive file size differs from inventory"));
        }
        let target = destination.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = private_file(&target)?;
        copy_pinned(&mut entry, &mut output, file.bytes, &file.sha256)?;
        output.sync_all()?;
        found.insert(path.to_owned());
    }
    if found != expected.keys().map(|p| (*p).to_owned()).collect() {
        return Err(invalid("archive lacks expected files"));
    }
    // Consume the gzip trailer, checking CRC and rejecting nonzero tar trailers/concatenated data.
    let mut reader = archive.into_inner();
    let mut buffer = [0; 8192];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        if buffer[..n].iter().any(|b| *b != 0) {
            return Err(invalid("unexpected data after tar terminator"));
        }
    }
    let decoded_bytes = reader.seen;
    let mut compressed = reader.reader.into_inner();
    if !compressed.fill_buf()?.is_empty() {
        return Err(invalid("multiple gzip members or trailing compressed data"));
    }
    if decoded_bytes % 512 != 0 || decoded_bytes < add(last_end, 1024)? {
        return Err(invalid("missing or malformed tar terminator"));
    }
    Ok(())
}
fn parents<'a>(paths: impl Iterator<Item = &'a str>) -> BTreeSet<String> {
    let mut dirs = BTreeSet::new();
    for path in paths {
        let mut parent = Path::new(path).parent();
        while let Some(p) = parent.filter(|p| !p.as_os_str().is_empty()) {
            dirs.insert(p.to_str().expect("validated UTF-8").to_owned());
            parent = p.parent();
        }
    }
    dirs
}
fn require_dir(path: &Path) -> io::Result<()> {
    if !fs::symlink_metadata(path)?.is_dir() {
        return Err(invalid("expected a directory without symlinks"));
    }
    Ok(())
}
fn walk(
    path: &Path,
    prefix: &str,
    files: &mut BTreeSet<String>,
    dirs: &mut BTreeSet<String>,
    expected: &BTreeSet<String>,
    expected_dirs: &BTreeSet<String>,
) -> io::Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid("non-UTF-8 installed path"))?;
        let relative = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.is_dir() {
            if !expected_dirs.contains(&relative) {
                return Err(invalid("unexpected installed directory"));
            }
            dirs.insert(relative.clone());
            walk(
                &entry.path(),
                &relative,
                files,
                dirs,
                expected,
                expected_dirs,
            )?;
        } else if metadata.is_file() && metadata.nlink() == 1 {
            if !expected.contains(&relative) {
                return Err(invalid("unexpected installed file"));
            }
            files.insert(relative);
        } else {
            return Err(invalid("installed links and special files are forbidden"));
        }
    }
    Ok(())
}
fn verify_files(directory: &Path, plan: &InstallPlan) -> io::Result<()> {
    require_dir(directory)?;
    let mut expected = BTreeSet::new();
    for archive in &plan.archives {
        for file in &archive.files {
            expected.insert(format!("{}/{}", archive.component, file.path));
        }
    }
    let mut actual = BTreeSet::new();
    let mut dirs = BTreeSet::new();
    walk(
        directory,
        "",
        &mut actual,
        &mut dirs,
        &expected,
        &parents(expected.iter().map(String::as_str)),
    )?;
    if actual != expected || dirs != parents(expected.iter().map(String::as_str)) {
        return Err(invalid("installed inventory differs from trusted plan"));
    }
    for archive in &plan.archives {
        for file in &archive.files {
            let path = directory.join(&archive.component).join(&file.path);
            let mut input = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&path)?;
            if input.metadata()?.len() != file.bytes {
                return Err(invalid("installed size mismatch"));
            }
            copy_pinned(&mut input, &mut io::sink(), file.bytes, &file.sha256)?;
        }
    }
    Ok(())
}

fn sync_directories(root: &Path) -> io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if fs::symlink_metadata(entry.path())?.is_dir() {
            sync_directories(&entry.path())?;
        }
    }
    File::open(root)?.sync_all()
}

fn available_space(root: &Path) -> io::Result<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let path =
        CString::new(root.as_os_str().as_bytes()).map_err(|_| invalid("NUL in store path"))?;
    let mut status = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: path is NUL terminated and status has the required writable layout.
    if unsafe { libc::statvfs(path.as_ptr(), status.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let status = unsafe { status.assume_init() };
    status
        .f_bavail
        .checked_mul(status.f_frsize)
        .ok_or_else(|| invalid("filesystem size overflow"))
}
fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let from =
        CString::new(from.as_os_str().as_bytes()).map_err(|_| invalid("NUL in staging path"))?;
    let to =
        CString::new(to.as_os_str().as_bytes()).map_err(|_| invalid("NUL in installed path"))?;
    // SAFETY: paths are NUL terminated. Both OS calls refuse any existing destination.
    #[cfg(target_os = "linux")]
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    #[cfg(target_os = "macos")]
    let result = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    return Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic install supports Linux and macOS",
    ));
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if result == 0 {
        Ok(())
    } else {
        let error = io::Error::last_os_error();
        if matches!(
            error.raw_os_error(),
            Some(libc::EINVAL | libc::ENOSYS | libc::EOPNOTSUPP)
        ) {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("database filesystem lacks atomic no-replace rename: {error}"),
            ))
        } else {
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests;
