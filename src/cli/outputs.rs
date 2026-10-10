//! Output paths of one run: refused when they collide, and never replacing an earlier result.
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

/// Everything a run writes, and the inputs it must not write into.
pub(crate) struct Paths<'a> {
    pub report: Option<&'a Path>,
    pub report_dir: Option<&'a Path>,
    pub anim_output: Option<&'a Path>,
    pub anim_cache: Option<&'a Path>,
    /// Package directories; their inventories reject any unlisted file.
    pub packages: Vec<&'a Path>,
    pub assemblies: Vec<&'a Path>,
}

/// Refuse a run whose outputs would overwrite, contain or sit inside one another, an input
/// package or an assembly. Checked before any file is written.
pub(crate) fn check(paths: &Paths<'_>) -> Result<(), String> {
    if let Some(report) = paths.report {
        if fs::symlink_metadata(report).is_ok() {
            return Err(format!(
                "report file {} already exists; an earlier result is never replaced",
                report.display()
            ));
        }
    }
    let outputs: Vec<(&str, &Path, PathBuf)> = [
        ("report file", paths.report),
        ("report directory", paths.report_dir),
        ("ANIm evidence directory", paths.anim_output),
        ("ANIm cache", paths.anim_cache),
    ]
    .into_iter()
    .filter_map(|(name, path)| path.map(|p| (name, p)))
    .map(|(name, path)| Ok((name, path, resolve(path)?)))
    .collect::<io::Result<_>>()
    .map_err(|e| e.to_string())?;
    for (i, (name, path, resolved)) in outputs.iter().enumerate() {
        for (other, other_path, other_resolved) in &outputs[i + 1..] {
            if resolved.starts_with(other_resolved) || other_resolved.starts_with(resolved) {
                return Err(format!(
                    "output collision: {name} {} and {other} {} overlap",
                    path.display(),
                    other_path.display()
                ));
            }
        }
        for package in &paths.packages {
            if resolved.starts_with(resolve(package).map_err(|e| e.to_string())?) {
                return Err(format!(
                    "output collision: {name} {} is inside package {}",
                    path.display(),
                    package.display()
                ));
            }
        }
        for assembly in &paths.assemblies {
            if *resolved == resolve(assembly).map_err(|e| e.to_string())? {
                return Err(format!(
                    "output collision: {name} {} is the assembly {}",
                    path.display(),
                    assembly.display()
                ));
            }
        }
    }
    // After the collision checks, which explain a report inside a directory the run would create.
    if let Some(report) = paths.report {
        if !directory_of(report).is_dir() {
            return Err(format!(
                "the directory of report file {} does not exist",
                report.display()
            ));
        }
    }
    Ok(())
}

/// An absolute path with symbolic links resolved for every existing ancestor, and `.` and
/// `..` applied to the rest, so two spellings of one location compare equal.
fn resolve(path: &Path) -> io::Result<PathBuf> {
    let absolute = std::env::current_dir()?.join(path);
    let mut resolved = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => resolved.push(component),
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            Component::Normal(name) => {
                resolved.push(name);
                if let Ok(canonical) = fs::canonicalize(&resolved) {
                    resolved = canonical;
                }
            }
        }
    }
    Ok(resolved)
}

/// Write `bytes` to a file that must not exist. The complete content is linked into place
/// in one step, so a reader never sees a partial report and an existing file is never
/// replaced, even one created during the run.
pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "report path has no name"))?;
    let directory = directory_of(path);
    let mut partial = OsString::from(".");
    partial.push(name);
    partial.push(format!(".tero-partial-{}", std::process::id()));
    let partial = directory.join(partial);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)?;
    let result = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| fs::hard_link(&partial, path));
    let _ = fs::remove_file(&partial);
    result
}

/// The directory a file path names, `.` for a bare file name.
fn directory_of(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}
