//! Command-line front ends: argument parsing, files and exit codes.
//!
//! Nothing in here decides anything. Decisions live in [`crate::decision`], which never
//! touches the terminal, the filesystem or the process.

use std::path::PathBuf;
use std::process::ExitCode;

use crate::decision::V1_POLICY_ID;
use crate::rescue::{Thresholds, V2_POLICY_ID};
use clap::Parser;

/// Shared threshold flags for native and saved-evidence front ends.
#[derive(Debug, Default, clap::Args)]
pub(crate) struct ThresholdArgs {
    /// Custom v2 species ANI percentage; always marks results custom_unvalidated
    #[arg(long, value_name = "T")]
    species_ani: Option<String>,
    /// Custom v2 subspecies ANI percentage; requires 80 <= T <= S <= 100
    #[arg(long, value_name = "S")]
    subspecies_ani: Option<String>,
}

impl ThresholdArgs {
    pub(crate) fn for_policy(&self, policy: &str) -> Result<Thresholds, String> {
        if self.species_ani.is_none() && self.subspecies_ani.is_none() {
            return Ok(Thresholds::standard());
        }
        if policy == V1_POLICY_ID {
            return Err(format!("--species-ani and --subspecies-ani need {V2_POLICY_ID}; v1 rejects threshold overrides"));
        }
        Thresholds::custom(self.species_ani.as_deref(), self.subspecies_ani.as_deref())
            .map_err(|e| e.to_string())
    }
}

// The public command, shared by the `tero` and `terobacter` executables. The command name is
// fixed so `--version` always names Terobacter; usage lines still show whichever name the user
// typed, because clap takes that from the executable.
#[derive(Debug, Parser)]
#[command(name = "terobacter", version, about)]
struct Public {
    /// Calling policy; threshold overrides are supported only by v2
    #[arg(long, default_value = V1_POLICY_ID, value_parser = [V1_POLICY_ID, V2_POLICY_ID])]
    assignment_policy: String,
    #[command(flatten)]
    thresholds: ThresholdArgs,
    /// Isolate assembly FASTA files, analysed whole and as supplied
    #[arg(value_name = "ASSEMBLY", required = true, num_args = 1..)]
    assemblies: Vec<PathBuf>,
}

/// Run the public command: `tero [OPTIONS] <ASSEMBLY>...`, with no subcommands.
pub fn run_public() -> ExitCode {
    if let Some(status) = crate::anim::run_worker_if_requested() {
        return status;
    }
    let args = Public::parse();
    if let Err(error) = args.thresholds.for_policy(&args.assignment_policy) {
        eprintln!("error: {error}");
        return ExitCode::FAILURE;
    }
    eprintln!(
        "error: whole-assembly classification is not implemented in this build \
         ({} assembly file(s) were not read)",
        args.assemblies.len()
    );
    ExitCode::FAILURE
}
