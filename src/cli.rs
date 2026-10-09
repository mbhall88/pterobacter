//! Command-line front ends: argument parsing, files and exit codes.
//!
//! Nothing in here decides anything. Decisions live in [`crate::decision`], which never
//! touches the terminal, the filesystem or the process.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
#[cfg(feature = "dev")]
use clap::Subcommand;

// The public command, shared by the `tero` and `terobacter` executables. The command name is
// fixed so `--version` always names Terobacter; usage lines still show whichever name the user
// typed, because clap takes that from the executable.
#[derive(Debug, Parser)]
#[command(name = "terobacter", version, about)]
struct Public {
    /// Isolate assembly FASTA files, analysed whole and as supplied
    #[arg(value_name = "ASSEMBLY", required = true, num_args = 1..)]
    assemblies: Vec<PathBuf>,
}

/// Run the public command: `tero [OPTIONS] <ASSEMBLY>...`, with no subcommands.
pub fn run_public() -> ExitCode {
    let args = Public::parse();
    eprintln!(
        "error: whole-assembly classification is not implemented in this build \
         ({} assembly file(s) were not read)",
        args.assemblies.len()
    );
    ExitCode::FAILURE
}

// Evaluation-only operations (R2-R5). They sit behind the `dev` feature in their own binary
// and share this crate's implementation, so a default build or install never exposes them.
#[cfg(feature = "dev")]
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

#[cfg(feature = "dev")]
#[derive(Debug, Subcommand)]
enum DevCommand {
    /// Decide the v1 species call from a saved 0.2.0 evidence file and print it as JSON
    SavedEvidence {
        /// Saved evidence JSON (schema 2.0.0-dev-evidence)
        evidence: PathBuf,
    },
}

/// Run the development binary.
#[cfg(feature = "dev")]
pub fn run_dev() -> ExitCode {
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

#[cfg(feature = "dev")]
fn classify_saved_evidence(path: &std::path::Path) -> Result<String, String> {
    use crate::decision::{decide, Policy};
    use crate::evidence::SavedEvidence;

    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let evidence =
        SavedEvidence::from_json(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(decide(&evidence, &Policy::v1()).to_json())
}
