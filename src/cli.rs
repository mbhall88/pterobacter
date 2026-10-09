//! Command-line front ends: argument parsing, files and exit codes.
//!
//! Nothing in here decides anything. Decisions live in [`crate::decision`], which never
//! touches the terminal, the filesystem or the process.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

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
