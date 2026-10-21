//! Repository hygiene: no workspace-only paths or private study directories in tracked files.
//!
//! The patterns are assembled from pieces so this file does not contain them itself.

use std::path::Path;
use std::process::Command;

fn forbidden() -> Vec<String> {
    vec![
        ["/scr", "atch/"].concat(),
        ["/ho", "me/"].concat(),
        ["uqm", "hal11"].concat(),
        ["Moni", "queC"].concat(),
    ]
}

#[test]
fn tracked_files_contain_no_workspace_paths_or_private_study_names() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let listing = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output()
        .expect("this test needs git to list tracked files");
    assert!(listing.status.success(), "git ls-files failed");

    let patterns = forbidden();
    let mut offenders = Vec::new();
    for name in listing.stdout.split(|b| *b == 0).filter(|n| !n.is_empty()) {
        let name = String::from_utf8_lossy(name).into_owned();
        let Ok(bytes) = std::fs::read(root.join(&name)) else {
            continue; // listed but deleted in the working tree
        };
        // The public reference licence quotes NCBI's policy URL in real reports.
        // Exempt that exact URL only; local paths elsewhere still fail.
        let text = String::from_utf8_lossy(&bytes).replace(
            "https://www.ncbi.nlm.nih.gov/home/about/policies/",
            "[NCBI molecular-data policy URL]",
        );
        offenders.extend(
            patterns
                .iter()
                .filter(|p| text.contains(p.as_str()))
                .map(|p| format!("{name}: {p}")),
        );
    }

    assert!(
        offenders.is_empty(),
        "workspace-only content found:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn public_policy_url_exception_does_not_hide_a_local_path() {
    let url = "https://www.ncbi.nlm.nih.gov/home/about/policies/";
    let local = ["/ho", "me/someone/data.fa"].concat();
    let cleaned = format!("{url} {local}").replace(url, "[NCBI molecular-data policy URL]");
    assert!(forbidden().iter().any(|pattern| cleaned.contains(pattern)));
    assert!(!forbidden()
        .iter()
        .any(|pattern| url.replace(url, "").contains(pattern)));
}
