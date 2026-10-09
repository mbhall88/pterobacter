//! Development binary: evaluation-only operations (saved-evidence classification, reference
//! package authentication and artifact verification). Compiled only with the `dev` feature, which
//! the module declaration in `lib.rs` gates, so a default build or install never contains any of it.
//! Failure produces no decision on stdout.

use crate::decision::{decide, decide_bound, Policy};
use crate::evidence::SavedEvidence;
use crate::reference::{
    is_sha256, verify_package, PackageError, PackageErrorKind, PackageSource, RegistryPins,
    VerifiedPackage,
};
use clap::{Args, Parser, Subcommand};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt::Display;
use std::fs::{self, File};
use std::io::{self, Read};
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
    SavedEvidence(SavedEvidenceArgs),
    /// Verify a reference package directory and print its identity and components as JSON
    ReferencePackage {
        /// Directory holding portable.json and the files it lists
        directory: PathBuf,
        /// SHA-256 that the package's portable.json must have
        #[arg(long, value_name = "SHA256", value_parser = parse_sha256)]
        expected_sha256: String,
        /// SHA-256 that the named registry table must have, if it is to be pinned
        #[arg(long, value_name = "SHA256", value_parser = parse_sha256)]
        named_registry_sha256: Option<String>,
        /// SHA-256 that the lineage registry table must have, if it is to be pinned
        #[arg(long, value_name = "SHA256", value_parser = parse_sha256)]
        lineage_registry_sha256: Option<String>,
    },
}

#[derive(Debug, Args)]
struct SavedEvidenceArgs {
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
    /// Verify this reference package and require the evidence to be bound to it
    #[arg(long, value_name = "DIR", requires = "expected_sha256")]
    reference_package: Option<PathBuf>,
    /// SHA-256 that the package's portable.json must have
    #[arg(
        long,
        value_name = "SHA256",
        value_parser = parse_sha256,
        requires = "reference_package"
    )]
    expected_sha256: Option<String>,
    /// SHA-256 that the named registry table must have, if it is to be pinned
    #[arg(
        long,
        value_name = "SHA256",
        value_parser = parse_sha256,
        requires = "reference_package"
    )]
    named_registry_sha256: Option<String>,
    /// SHA-256 that the lineage registry table must have, if it is to be pinned
    #[arg(
        long,
        value_name = "SHA256",
        value_parser = parse_sha256,
        requires = "reference_package"
    )]
    lineage_registry_sha256: Option<String>,
}

fn parse_sha256(text: &str) -> Result<String, String> {
    if is_sha256(text) {
        Ok(text.to_string())
    } else {
        Err("expected 64 lowercase hexadecimal characters".to_string())
    }
}

const EXIT_FAILURE: u8 = 1;
const EXIT_MISSING: u8 = 3;
const EXIT_ALTERED: u8 = 4;
const EXIT_TRUNCATED: u8 = 5;
const EXIT_INCOMPATIBLE: u8 = 6;
const EXIT_INCONSISTENT: u8 = 7;
const EXIT_UNRESOLVED: u8 = 8;
const EXIT_UNEXPECTED: u8 = 9;
const EXIT_UNLISTED: u8 = 10;

struct Failure {
    code: u8,
    message: String,
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Failure {
            code: EXIT_FAILURE,
            message,
        }
    }
}

impl From<PackageError> for Failure {
    fn from(error: PackageError) -> Self {
        let code = match error.kind() {
            PackageErrorKind::Io => EXIT_FAILURE,
            PackageErrorKind::Unexpected => EXIT_UNEXPECTED,
            PackageErrorKind::Missing => EXIT_MISSING,
            PackageErrorKind::Altered => EXIT_ALTERED,
            PackageErrorKind::Truncated => EXIT_TRUNCATED,
            PackageErrorKind::Incompatible => EXIT_INCOMPATIBLE,
            PackageErrorKind::Inconsistent => EXIT_INCONSISTENT,
            PackageErrorKind::Unresolved => EXIT_UNRESOLVED,
            PackageErrorKind::Unlisted => EXIT_UNLISTED,
        };
        Failure {
            code,
            message: error.to_string(),
        }
    }
}

/// Run the development binary.
pub fn run() -> ExitCode {
    let Dev { command } = Dev::parse();
    let result = match command {
        DevCommand::SavedEvidence(args) => classify(args),
        DevCommand::ReferencePackage {
            directory,
            expected_sha256,
            named_registry_sha256,
            lineage_registry_sha256,
        } => reference_package(
            &directory,
            &expected_sha256,
            &RegistryPins {
                named_registry_sha256,
                lineage_registry_sha256,
            },
        ),
    };
    match result {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(failure) => {
            eprintln!("error: {}", failure.message);
            ExitCode::from(failure.code)
        }
    }
}

fn reference_package(
    directory: &Path,
    expected_sha256: &str,
    pins: &RegistryPins,
) -> Result<String, Failure> {
    let package: VerifiedPackage =
        verify_package(&Directory(directory.to_path_buf()), expected_sha256, pins)?;
    Ok(serde_json::to_string_pretty(&package.to_value()).expect("a JSON value always serialises"))
}

/// Reads a package directory. Symbolic links and names that are not UTF-8 are refused.
struct Directory(PathBuf);

impl PackageSource for Directory {
    fn open(&self, path: &str) -> io::Result<Option<Box<dyn Read + '_>>> {
        let mut full = self.0.clone();
        for segment in path.split('/') {
            full.push(segment);
            match fs::symlink_metadata(&full) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(symlink_refused());
                }
                Ok(_) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                    ) =>
                {
                    return Ok(None);
                }
                Err(error) => return Err(error),
            }
        }
        let file = File::open(full)?;
        Ok(Some(Box::new(file)))
    }

    fn inventory(&self) -> io::Result<BTreeSet<String>> {
        let mut found = BTreeSet::new();
        list_files(&self.0, "", &mut found)?;
        Ok(found)
    }
}

fn list_files(directory: &Path, prefix: &str, found: &mut BTreeSet<String>) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name().into_string().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "a file name in the package is not UTF-8",
            )
        })?;
        let relative = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(symlink_refused());
        }
        if file_type.is_dir() {
            list_files(&entry.path(), &relative, found)?;
        } else {
            found.insert(relative);
        }
    }
    Ok(())
}

fn symlink_refused() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "symbolic links are not allowed in a package",
    )
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

fn classify(args: SavedEvidenceArgs) -> Result<String, Failure> {
    let SavedEvidenceArgs {
        evidence,
        evidence_sha256,
        diagnostics,
        diagnostics_sha256,
        reference_inventory,
        reference_inventory_sha256,
        problem_warnings,
        problem_warnings_sha256,
        reference_package,
        expected_sha256,
        named_registry_sha256,
        lineage_registry_sha256,
    } = args;
    let mut pins = Vec::new();
    let text = read(&evidence, evidence_sha256.as_deref(), &mut pins)?;
    let saved =
        SavedEvidence::from_json(&text).map_err(|e| format!("{}: {e}", evidence.display()))?;
    let mut optional =
        |path: Option<PathBuf>, expected: Option<String>| -> Result<Option<String>, String> {
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
    let (decision, verification) = match reference_package.zip(expected_sha256) {
        Some((directory, expected)) => {
            let registry_pins = RegistryPins {
                named_registry_sha256,
                lineage_registry_sha256,
            };
            let package = verify_package(&Directory(directory), &expected, &registry_pins)?;
            (
                decide_bound(&saved, &package, &Policy::v1())?,
                "input_and_artifact_hashes_checked; reference_package_verified",
            )
        }
        None => (
            decide(&saved, &Policy::v1()),
            "input_and_artifact_hashes_checked; package_authentication_requires_checked_inventory",
        ),
    };
    let mut result = decision.to_value().clone();
    result["input"]["verification"] = Value::String(verification.into());
    let output = serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?;
    // Recheck the originally pinned bytes before a successful result can leave this wrapper.
    for (path, hash) in &pins {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if format!("{:x}", Sha256::digest(bytes)) != *hash {
            return Err(format!("{}: input changed during decision", path.display()).into());
        }
    }
    Ok(output)
}
