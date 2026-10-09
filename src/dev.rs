//! Development binary: evaluation-only operations (saved-evidence classification and reference
//! package authentication). Compiled only with the `dev` feature, which the module declaration in
//! `lib.rs` gates, so a default build or install never contains any of it.

use std::fmt::Display;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::decision::{decide, Policy};
use crate::evidence::SavedEvidence;
use crate::reference::{
    verify_package, PackageError, PackageErrorKind, PackageSource, VerifiedPackage,
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
        #[arg(long, value_name = "DIR")]
        reference_package: Option<PathBuf>,
    },
    /// Verify a reference package directory and print its identity and components as JSON
    ReferencePackage {
        /// Directory holding portable.json and the files it lists
        directory: PathBuf,
    },
}

const EXIT_FAILURE: u8 = 1;
const EXIT_MISSING: u8 = 3;
const EXIT_ALTERED: u8 = 4;
const EXIT_TRUNCATED: u8 = 5;
const EXIT_INCOMPATIBLE: u8 = 6;
const EXIT_INCOMPLETE: u8 = 7;
const EXIT_NOT_BOUND: u8 = 8;

struct Failure {
    code: u8,
    message: String,
}

impl From<PackageError> for Failure {
    fn from(error: PackageError) -> Self {
        let code = match error.kind() {
            PackageErrorKind::Io => EXIT_FAILURE,
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
        } => saved_evidence(&evidence, reference_package.as_deref()),
        DevCommand::ReferencePackage { directory } => reference_package(&directory),
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

fn saved_evidence(path: &Path, package_dir: Option<&Path>) -> Result<String, Failure> {
    let fail = |error: &dyn Display| Failure {
        code: EXIT_FAILURE,
        message: format!("{}: {error}", path.display()),
    };
    let text = std::fs::read_to_string(path).map_err(|e| fail(&e))?;
    let evidence = SavedEvidence::from_json(&text).map_err(|e| fail(&e))?;
    let package = package_dir
        .map(|dir| verify_package(&Directory(dir.to_path_buf())))
        .transpose()?;
    let decision = decide(&evidence, package.as_ref(), &Policy::v1())?;
    Ok(decision.to_json())
}

fn reference_package(directory: &Path) -> Result<String, Failure> {
    let package: VerifiedPackage = verify_package(&Directory(directory.to_path_buf()))?;
    Ok(serde_json::to_string_pretty(&package.to_value()).expect("a JSON value always serialises"))
}

/// The verifier checks each listed path for safety before opening it, so reads stay inside the
/// directory.
struct Directory(PathBuf);

impl PackageSource for Directory {
    fn open(&self, path: &str) -> io::Result<Option<Box<dyn Read + '_>>> {
        match File::open(self.0.join(path)) {
            Ok(file) => Ok(Some(Box::new(file))),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }
}
