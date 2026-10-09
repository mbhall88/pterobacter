//! Development binary: evaluation-only operations (saved-evidence classification and later
//! diagnostics). Compiled only with the `dev` feature, which the module declaration in
//! `lib.rs` gates, so a default build or install never contains any of it.

use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::decision::{decide, Policy};
use crate::evidence::SavedEvidence;
use crate::skani::{SearchSettings, SeedingMode, SkaniDatabase};

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

/// Run the development binary.
pub fn run() -> ExitCode {
    let result = match Dev::parse().command {
        DevCommand::SavedEvidence { evidence } => classify_saved_evidence(&evidence),
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
    };
    match result {
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
