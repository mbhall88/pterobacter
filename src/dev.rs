//! Development binary: evaluation-only operations (saved-evidence classification and later
//! diagnostics). Compiled only with the `dev` feature, which the module declaration in
//! `lib.rs` gates, so a default build or install never contains any of it.

use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::decision::{decide, Policy};
use crate::evidence::SavedEvidence;

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
    },
}

/// Run the development binary.
pub fn run() -> ExitCode {
    let Dev {
        command: DevCommand::SavedEvidence { evidence },
    } = Dev::parse();
    match classify_saved_evidence(&evidence) {
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

fn classify_saved_evidence(path: &Path) -> Result<String, String> {
    let fail = |error: &dyn Display| format!("{}: {error}", path.display());
    let text = std::fs::read_to_string(path).map_err(|e| fail(&e))?;
    let evidence = SavedEvidence::from_json(&text).map_err(|e| fail(&e))?;
    Ok(decide(&evidence, &Policy::v1()).to_json())
}
