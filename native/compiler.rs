//! Native compiler selection. Override ordering matches the locked cc-rs 1.4.0.
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub fn override_keys(host: &str, target: &str) -> [String; 4] {
    let kind = if host == target { "HOST" } else { "TARGET" };
    [
        format!("CXX_{target}"),
        format!("CXX_{}", target.replace(['-', '.'], "_")),
        format!("{kind}_CXX"),
        "CXX".into(),
    ]
}

pub fn explicit_override(
    host: &str,
    target: &str,
    mut get: impl FnMut(&str) -> Option<OsString>,
) -> Result<Option<String>, String> {
    for key in override_keys(host, target) {
        if let Some(value) = get(&key) {
            // cc-rs trims its selected value and otherwise falls back to a default.
            // Reject that fallback while leaving nonblank wrapper/argument parsing to cc-rs.
            if value.to_string_lossy().trim().is_empty() {
                return Err(format!(
                    "Invalid GNU C++ compiler override {key}: value is empty or whitespace; set it to a GNU C++ compiler or unset the variable"
                ));
            }
            return Ok(Some(key));
        }
    }
    Ok(None)
}

pub fn needs_discovery(host: &str, target: &str, explicit: bool) -> Result<bool, String> {
    if explicit {
        return Ok(false);
    }
    if host != target && (host.contains("apple-darwin") || target.contains("apple-darwin")) {
        return Err("Cross-compilation requires an explicit target GNU C++ compiler: set a target-specific CXX override or TARGET_CXX; Homebrew host discovery is disabled".into());
    }
    Ok(host == target && target.contains("apple-darwin"))
}

pub fn discover(prefixes: &[PathBuf], mut is_gnu: impl FnMut(&Path) -> bool) -> Option<PathBuf> {
    for prefix in prefixes {
        let Ok(entries) = fs::read_dir(prefix.join("bin")) else {
            continue;
        };
        let mut candidates: Vec<_> = entries
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                let name = path.file_name()?.to_str()?;
                let version = name.strip_prefix("g++-")?.parse::<u32>().ok()?;
                Some((version, path))
            })
            .collect();
        candidates.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        for (_, candidate) in candidates {
            if is_gnu(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn homebrew_prefixes() -> Vec<PathBuf> {
    let mut prefixes = Vec::new();
    for brew in ["brew", "/opt/homebrew/bin/brew", "/usr/local/bin/brew"] {
        if let Ok(output) = Command::new(brew).args(["--prefix", "gcc"]).output() {
            if output.status.success() {
                if let Ok(value) = String::from_utf8(output.stdout) {
                    let prefix = PathBuf::from(value.trim());
                    if prefix.is_absolute() && !prefixes.contains(&prefix) {
                        prefixes.push(prefix);
                    }
                }
            }
        }
    }
    for prefix in ["/opt/homebrew/opt/gcc", "/usr/local/opt/gcc"] {
        let prefix = PathBuf::from(prefix);
        if !prefixes.contains(&prefix) {
            prefixes.push(prefix);
        }
    }
    prefixes
}

pub fn is_gnu(path: &Path) -> bool {
    // Clang defines __GNUC__ for compatibility, so exclude it explicitly.
    Command::new(path)
        .args(["-dM", "-E", "-x", "c++", "/dev/null"])
        .output()
        .is_ok_and(|output| {
            let macros = String::from_utf8_lossy(&output.stdout);
            output.status.success()
                && macros.contains("#define __GNUC__ ")
                && !macros.contains("#define __clang__ ")
        })
}

/// Select the runtime explicitly; an active Conda environment alone changes nothing.
pub fn cpp_stdlib(target: &str, value: Option<OsString>) -> Result<&'static str, String> {
    match value.as_deref().and_then(|s| s.to_str()) {
        None if value.is_none() => Ok("stdc++"),
        Some("stdc++") => Ok("stdc++"),
        Some("c++") if target.contains("apple-darwin") => Ok("c++"),
        _ => Err("PTERO_CXX_STDLIB must be stdc++, or c++ for a macOS GNU/libc++ toolchain".into()),
    }
}

pub fn alternate_string_layout(target: &str, stdlib: &str) -> bool {
    target == "aarch64-apple-darwin" && stdlib == "c++"
}
