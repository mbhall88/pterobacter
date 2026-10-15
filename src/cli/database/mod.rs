//! Installer transport primitives. No public catalogue format or command is committed here.
//!
//! A reviewed catalogue adapter must supply the pins independently of the downloaded payload.
//! Scientific loaders remain responsible for verifying component compatibility before publication.
pub mod install;

use std::io;
use std::path::{Path, PathBuf};

/// Select only an explicitly requested release or an independently pinned exact default.
/// The installed directory contents never determine selection.
pub fn select<'a>(
    plans: &'a [install::InstallPlan],
    requested: Option<&str>,
    default: Option<&str>,
) -> io::Result<&'a install::InstallPlan> {
    let version = requested.or(default).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "no database version or pinned default selected",
        )
    })?;
    let mut seen = std::collections::BTreeSet::new();
    for plan in plans {
        plan.validate()?;
        if !seen.insert(&plan.version) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate database version",
            ));
        }
    }
    plans.iter().find(|p| p.version == version).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("database version {version:?} is not in the trusted catalogue"),
        )
    })
}

/// Reserve a retained evidence directory in a private state root. Existing directories are
/// never reused or removed. The caller must report the returned location to the user.
pub fn reserve_evidence(state_root: &Path) -> io::Result<PathBuf> {
    install::reserve_evidence(state_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_requires_an_exact_pin_even_with_one_available_version() {
        let mut plan = install::InstallPlan {
            version: "fixture-1".into(),
            archives: vec![],
        };
        // Use a valid transport plan; the path/digests need no archive for selection.
        plan.archives.push(install::ArchivePin {
            component: "primary".into(),
            archive_root: None,
            bytes: 1,
            sha256: "a".repeat(64),
            tar_bytes_limit: 2048,
            files: vec![install::FilePin {
                path: "data".into(),
                bytes: 1,
                sha256: "b".repeat(64),
            }],
        });
        let mut other = plan.clone();
        other.version = "fixture-2".into();
        let plans = [plan, other];
        assert!(select(&plans, None, None).is_err());
        assert!(select(&plans[..1], None, None).is_err());
        assert!(select(&plans, Some("latest"), None).is_err());
        assert!(select(&plans, None, Some("absent")).is_err());
        assert_eq!(
            select(&plans, Some("fixture-1"), Some("fixture-2"))
                .unwrap()
                .version,
            "fixture-1"
        );
        assert_eq!(
            select(&plans, None, Some("fixture-2")).unwrap().version,
            "fixture-2"
        );
        let duplicates = [plans[0].clone(), plans[0].clone()];
        assert!(select(&duplicates, Some("fixture-1"), None).is_err());
    }

    #[test]
    fn evidence_reservations_are_concurrent_unique_and_retained() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let paths = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..16)
                .map(|_| scope.spawn(|| reserve_evidence(root.path()).unwrap()))
                .collect();
            workers
                .into_iter()
                .map(|w| w.join().unwrap())
                .collect::<std::collections::BTreeSet<_>>()
        });
        assert_eq!(paths.len(), 16);
        let first = paths.first().unwrap();
        std::fs::write(first.join("evidence"), b"retained bytes").unwrap();
        let next = reserve_evidence(root.path()).unwrap();
        assert!(!paths.contains(&next));
        assert_eq!(
            std::fs::read(first.join("evidence")).unwrap(),
            b"retained bytes"
        );
    }
}
