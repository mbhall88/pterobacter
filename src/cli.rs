//! Public command parsing, output and exit codes.
pub mod assembly;

use crate::decision::V1_POLICY_ID;
use crate::evidence::sha256_hex;
use crate::reference::PackageSource;
use clap::Parser;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(name = "terobacter", version, about)]
struct Public {
    /// Isolate assembly FASTA files, analysed whole and as supplied
    #[arg(value_name = "ASSEMBLY", required = true, num_args = 1..)]
    assemblies: Vec<PathBuf>,
    /// Authenticated reference package directory
    #[arg(long)]
    reference_package: PathBuf,
    /// Expected SHA-256 of portable.json, obtained from a trusted source
    #[arg(long, value_parser = parse_sha256)]
    expected_sha256: String,
    /// Native classification policy; this build supports v1 only
    #[arg(long, default_value = V1_POLICY_ID)]
    policy: String,
    /// Kind of input; assembly FASTA is the supported kind
    #[arg(long, default_value = "assembly")]
    input_kind: String,
    /// Worker threads for each assembly
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u16).range(1..))]
    threads: u16,
}
fn parse_sha256(text: &str) -> Result<String, String> {
    if crate::reference::is_sha256(text) {
        Ok(text.into())
    } else {
        Err("expected 64 lowercase hexadecimal characters".into())
    }
}

/// One implementation for both public executable names.
pub fn run_public() -> ExitCode {
    if let Some(status) = crate::anim::run_worker_if_requested() {
        return status;
    }
    let args = Public::parse();
    let mut results = Vec::new();
    let mut exit = 0;
    let mut workflow = None;
    for path in &args.assemblies {
        let mut report = json!({"schema": "terobacter.assembly-call/1",
            "implementation": {"name": env!("CARGO_PKG_NAME"), "version": env!("CARGO_PKG_VERSION")},
            "policy": {"id": args.policy},
            "input": {"kind": args.input_kind, "file": path.to_string_lossy(), "sequence_scope": "whole_assembly_as_supplied"},
            "assignment": {"status": "not_evaluated", "species_id": null, "species_name": null,
                "subspecies_id": null, "subspecies_name": null, "lineage_id": null, "genomovar_id": null},
            "execution": {"status": "incomplete", "stage": "input"}});
        if args.policy != V1_POLICY_ID {
            report["execution"]["stage"] = "policy".into();
            report["execution"]["error"] = "requested native policy is unavailable; only conservative-assembly-v1 is implemented".into();
            exit = 5;
            results.push(report);
            continue;
        }
        let result = (|| -> Result<(), (u8, String)> {
            if args.input_kind != "assembly" {
                return Err((3, "only assembly FASTA input is supported".into()));
            }
            let bytes = read_input(path, &mut report).map_err(|e| (3, e))?;
            report["execution"]["stage"] = "package".into();
            if workflow.is_none() {
                workflow = Some(
                    assembly::AssemblyWorkflow::open(
                        &args.reference_package,
                        &args.expected_sha256,
                    )
                    .map_err(|e| (4, e))?,
                );
            }
            workflow
                .as_ref()
                .expect("opened workflow")
                .analyse(
                    path.to_str().ok_or((3, "query path is not UTF-8".into()))?,
                    &bytes,
                    args.threads.into(),
                    &mut report,
                )
                .map_err(|e| (1, e))?;
            Ok(())
        })();
        if let Err((code, error)) = result {
            report["execution"]["error"] = error.into();
            exit = code;
        }
        results.push(report);
    }
    if let Some(workflow) = workflow {
        if let Err(error) = workflow.finish() {
            exit = 4;
            for report in &mut results {
                if report["execution"]["status"] == "complete" {
                    report["execution"] =
                        json!({"status": "incomplete", "stage": "package_recheck", "error": error});
                    report["assignment"] = json!({"status": "not_evaluated", "species_id": null, "species_name": null,
                        "subspecies_id": null, "subspecies_name": null, "lineage_id": null, "genomovar_id": null});
                    report["species"] = Value::Null;
                    report["subspecies"] = Value::Null;
                }
            }
        }
    }
    // Publication: a genomovar stands only if its parent is still the report's final call.
    for report in &mut results {
        crate::genomovar::recheck(report);
    }
    let value = if results.len() == 1 {
        results.remove(0)
    } else {
        Value::Array(results)
    };
    let result = (|| -> io::Result<()> {
        let mut out = io::stdout().lock();
        serde_json::to_writer_pretty(&mut out, &value)?;
        writeln!(out)
    })();
    if let Err(error) = result {
        eprintln!("error writing report: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::from(exit)
}

/// Reads a package directory. Symbolic links and names that are not UTF-8 are refused.
pub(crate) struct Directory(pub(crate) PathBuf);

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

pub(crate) fn read_input(path: &Path, report: &mut Value) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    report["input"]["query_sha256"] = sha256_hex(&bytes).into();
    report["input"]["bytes"] = bytes.len().into();
    report["input"]["records"] = json!([]);
    let mut reader = needletail::parse_fastx_reader(std::io::Cursor::new(bytes.clone()))
        .map_err(|e| e.to_string())?;
    let mut total = 0_usize;
    while let Some(record) = reader.next() {
        let record = record.map_err(|e| e.to_string())?;
        if record.format() != needletail::parser::Format::Fasta {
            return Err("input must be assembly FASTA, not FASTQ".into());
        }
        if record.id().is_empty() {
            return Err("FASTA record has no identifier".into());
        }
        let id = std::str::from_utf8(record.id()).map_err(|e| e.to_string())?;
        let seq = record.seq();
        if !crate::skani::is_iupac_sequence(&seq) {
            return Err("empty sequence or non-IUPAC nucleotide in assembly".into());
        }
        total = total
            .checked_add(seq.len())
            .ok_or("assembly length overflow")?;
        let ordinal = report["input"]["records"]
            .as_array()
            .expect("record list")
            .len()
            + 1;
        report["input"]["records"]
            .as_array_mut()
            .expect("record list")
            .push(json!({
            "ordinal": ordinal,
            "header": id, "length": seq.len(), "sequence_sha256": sha256_hex(&seq)}));
        report["input"]["total_bases"] = total.into();
    }
    if total == 0 {
        return Err("assembly contains no sequence".into());
    }
    Ok(bytes)
}
