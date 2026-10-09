//! Files, argument parsing and artifact verification for the feature-gated development binary.
use crate::decision::{decide, Policy};
use crate::evidence::SavedEvidence;
use crate::skani::{SearchSettings, SeedingMode, SkaniDatabase};
use clap::{Parser, Subcommand};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "terobacter-dev",
    version,
    about = "Evaluation-only operations. Not part of the public command"
)]
struct Dev {
    #[command(subcommand)]
    command: DevCommand,
}
#[derive(Debug, Subcommand)]
enum DevCommand {
    /// Decide v1 species/subspecies from saved 0.2.0 evidence; no new measurements
    SavedEvidence {
        /// Saved evidence JSON, schema 2.0.0-dev-evidence
        evidence: PathBuf,
        /// Optional expected SHA-256 of the exact saved evidence bytes
        #[arg(long)]
        evidence_sha256: Option<String>,
        /// Bound reciprocal ANIm diagnostic envelope
        #[arg(long, requires_all = ["diagnostics_sha256", "reference_inventory"])]
        diagnostics: Option<PathBuf>,
        #[arg(long, requires = "diagnostics")]
        diagnostics_sha256: Option<String>,
        /// Primary inventory from a checked reference package; required for method diagnostics
        #[arg(long, requires_all = ["diagnostics", "reference_inventory_sha256"])]
        reference_inventory: Option<PathBuf>,
        #[arg(long, requires = "reference_inventory")]
        reference_inventory_sha256: Option<String>,
        /// Bound external problem-warning envelope
        #[arg(long, requires = "problem_warnings_sha256")]
        problem_warnings: Option<PathBuf>,
        #[arg(long, requires = "problem_warnings")]
        problem_warnings_sha256: Option<String>,
    },
    /// Measure one assembly with the embedded skani compatibility profile
    SkaniSearch {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        query: PathBuf,
        #[arg(long, default_value_t = 1)]
        threads: usize,
        /// Use the genomovar zero reporting floor and retain every returned row
        #[arg(long)]
        genomovar: bool,
        /// Exercise the portable path even on an AVX2 CPU
        #[arg(long)]
        portable: bool,
    },
}
/// Run the development binary. Failure produces no decision on stdout.
pub fn run() -> ExitCode {
    let Dev { command } = Dev::parse();
    match classify(command) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}
fn read(
    path: &Path,
    expected: Option<&str>,
    pins: &mut Vec<(PathBuf, String)>,
) -> Result<String, String> {
    let fail = |error: &dyn Display| format!("{}: {error}", path.display());
    let bytes = std::fs::read(path).map_err(|e| fail(&e))?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    if expected.is_some_and(|digest| digest != hash) {
        return Err(format!("{}: SHA-256 mismatch", path.display()));
    }
    pins.push((path.to_path_buf(), hash));
    String::from_utf8(bytes).map_err(|e| fail(&e))
}
fn verify_artifact(
    path: &Path,
    expected: &str,
    pins: &mut Vec<(PathBuf, String)>,
) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if actual != expected {
        return Err(format!("{}: artifact SHA-256 mismatch", path.display()));
    }
    pins.push((path.to_path_buf(), actual));
    Ok(())
}
fn verify_artifacts(
    evidence: &SavedEvidence,
    pins: &mut Vec<(PathBuf, String)>,
) -> Result<(), String> {
    if let Some(checks) = evidence.scope()["checks"].as_array() {
        for check in checks {
            let path = Path::new(
                check["artifact_path"]
                    .as_str()
                    .expect("validated scope path"),
            );
            if !path.is_absolute() {
                return Err("scope artifacts must use absolute paths".into());
            }
            verify_artifact(
                path,
                check["sha256"].as_str().expect("validated scope hash"),
                pins,
            )?;
        }
    }
    for item in evidence
        .diagnostics()
        .iter()
        .chain(evidence.problem_warnings())
    {
        for pin in item["artifacts"]
            .as_array()
            .expect("validated artifact list")
        {
            verify_artifact(
                Path::new(pin["path"].as_str().expect("validated artifact path")),
                pin["sha256"].as_str().expect("validated artifact hash"),
                pins,
            )?;
        }
    }
    Ok(())
}
fn classify(command: DevCommand) -> Result<String, String> {
    match command {
        DevCommand::SavedEvidence {
            evidence,
            evidence_sha256,
            diagnostics,
            diagnostics_sha256,
            reference_inventory,
            reference_inventory_sha256,
            problem_warnings,
            problem_warnings_sha256,
        } => {
            let mut pins = Vec::new();
            let text = read(&evidence, evidence_sha256.as_deref(), &mut pins)?;
            let saved = SavedEvidence::from_json(&text)
                .map_err(|e| format!("{}: {e}", evidence.display()))?;
            let mut optional = |path: Option<PathBuf>,
                                expected: Option<String>|
             -> Result<Option<String>, String> {
                path.map(|path| read(&path, expected.as_deref(), &mut pins))
                    .transpose()
            };
            let method = optional(diagnostics, diagnostics_sha256)?;
            let inventory = optional(reference_inventory, reference_inventory_sha256)?;
            let warning = optional(problem_warnings, problem_warnings_sha256)?;
            let saved = saved
                .with_annotations(method.as_deref(), warning.as_deref(), inventory.as_deref())
                .map_err(|e| e.to_string())?;
            verify_artifacts(&saved, &mut pins)?;
            let mut result = decide(&saved, &Policy::v1()).to_value().clone();
            result["input"]["verification"] = Value::String(
        "input_and_artifact_hashes_checked; package_authentication_requires_checked_inventory"
            .into(),
    );
            let output = serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?;
            // Recheck the originally pinned bytes before a successful result can leave this wrapper.
            for (path, hash) in &pins {
                let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
                if format!("{:x}", Sha256::digest(bytes)) != *hash {
                    return Err(format!("{}: input changed during decision", path.display()));
                }
            }
            Ok(output)
        }
        DevCommand::SkaniSearch {
            database,
            query,
            threads,
            genomovar,
            portable,
        } => {
            let settings = if genomovar {
                SearchSettings::Genomovar
            } else {
                SearchSettings::Primary
            };
            let mode = if portable {
                SeedingMode::Portable
            } else {
                SeedingMode::Auto
            };
            SkaniDatabase::open(&database)
                .and_then(|db| db.search(&query, settings, threads, mode))
                .map_err(|e| e.to_string())
                .and_then(|report| serde_json::to_string_pretty(&report).map_err(|e| e.to_string()))
        }
    }
}
