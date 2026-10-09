//! Development binary: evaluation-only operations (saved-evidence classification and reference
//! package authentication). Compiled only with the `dev` feature, which the module declaration in
//! `lib.rs` gates, so a default build or install never contains any of it.

use std::fmt::Display;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::decision::{decide, Policy};
use crate::evidence::SavedEvidence;
use crate::reference::{
    is_sha256, verify_package, PackageError, PackageErrorKind, PackageSource, VerifiedPackage,
};

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
    /// Decide the v1 species call from a saved 0.2.0 evidence file and print it as JSON
    SavedEvidence {
        /// Saved evidence JSON (schema 2.0.0-dev-evidence)
        evidence: PathBuf,
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
    },
    /// Verify a reference package directory and print its identity and components as JSON
    ReferencePackage {
        /// Directory holding portable.json and the files it lists
        directory: PathBuf,
        /// SHA-256 that the package's portable.json must have
        #[arg(long, value_name = "SHA256", value_parser = parse_sha256)]
        expected_sha256: String,
    },
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
const EXIT_INCOMPLETE: u8 = 7;
const EXIT_NOT_BOUND: u8 = 8;
const EXIT_UNEXPECTED: u8 = 9;

struct Failure {
    code: u8,
    message: String,
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
            PackageErrorKind::Incomplete => EXIT_INCOMPLETE,
            PackageErrorKind::NotBound => EXIT_NOT_BOUND,
        };
        Failure {
            code,
            message: error.to_string(),
        }
    }
}

/// Run the development binary.
pub fn run() -> ExitCode {
    let result = match Dev::parse().command {
        DevCommand::SavedEvidence {
            evidence,
            reference_package,
            expected_sha256,
        } => saved_evidence(&evidence, reference_package.zip(expected_sha256)),
        DevCommand::ReferencePackage {
            directory,
            expected_sha256,
        } => reference_package(&directory, &expected_sha256),
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

fn saved_evidence(path: &Path, package: Option<(PathBuf, String)>) -> Result<String, Failure> {
    let fail = |error: &dyn Display| Failure {
        code: EXIT_FAILURE,
        message: format!("{}: {error}", path.display()),
    };
    let text = std::fs::read_to_string(path).map_err(|e| fail(&e))?;
    let evidence = SavedEvidence::from_json(&text).map_err(|e| fail(&e))?;
    let package = package
        .map(|(dir, expected)| verify_package(&Directory(dir), &expected))
        .transpose()?;
    let decision = decide(&evidence, package.as_ref(), &Policy::v1())?;
    Ok(decision.to_json())
}

fn reference_package(directory: &Path, expected_sha256: &str) -> Result<String, Failure> {
    let package: VerifiedPackage =
        verify_package(&Directory(directory.to_path_buf()), expected_sha256)?;
    Ok(serde_json::to_string_pretty(&package.to_value()).expect("a JSON value always serialises"))
}

/// Reads a package directory, refusing a symbolic link on any listed path.
struct Directory(PathBuf);

impl PackageSource for Directory {
    fn open(&self, path: &str) -> io::Result<Option<Box<dyn Read + '_>>> {
        let mut full = self.0.clone();
        for segment in path.split('/') {
            full.push(segment);
            match fs::symlink_metadata(&full) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "symbolic links are not allowed in a package",
                    ));
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
}
