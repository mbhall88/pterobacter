//! Public command parsing, output and exit codes.
pub mod assembly;
pub mod mlst;
pub mod v2;

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
    /// Native classification policy; v1 or conservative-assembly-anim-v2
    #[arg(long, default_value = V1_POLICY_ID)]
    policy: String,
    /// Kind of input; assembly FASTA is the supported kind
    #[arg(long, default_value = "assembly")]
    input_kind: String,
    /// Worker threads for each assembly
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u16).range(1..))]
    threads: u16,
    /// Historical MLST scheme package directory; requests optional supplementary MLST
    #[arg(long, requires = "mlst_expected_sha256")]
    mlst_package: Option<PathBuf>,
    /// Expected SHA-256 of the MLST package manifest.json, obtained from a trusted source
    #[arg(long, value_parser = parse_sha256, requires = "mlst_package")]
    mlst_expected_sha256: Option<String>,
    #[command(flatten)]
    anim: v2::V2Settings,
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
    if args.policy == crate::rescue::V2_POLICY_ID {
        install_cancel_handlers();
    }
    let output_error = if args.policy == crate::rescue::V2_POLICY_ID {
        args.anim
            .anim_output
            .as_ref()
            .and_then(|p| fs::create_dir(p).err())
            .map(|e| e.to_string())
    } else {
        None
    };
    let mut results = Vec::new();
    let mut exit = 0;
    let mut workflow = None;
    let mut mlst = args
        .mlst_package
        .as_deref()
        .zip(args.mlst_expected_sha256.as_deref())
        .map(|(directory, sha256)| mlst::Mlst::new(directory, sha256));
    for (index, path) in args.assemblies.iter().enumerate() {
        let mut report = json!({"schema": "terobacter.assembly-call/1",
            "implementation": {"name": env!("CARGO_PKG_NAME"), "version": env!("CARGO_PKG_VERSION")},
            "policy": {"id": args.policy},
            "input": {"kind": args.input_kind, "file": path.to_string_lossy(), "sequence_scope": "whole_assembly_as_supplied"},
            "assignment": {"status": "not_evaluated", "species_id": null, "species_name": null,
                "subspecies_id": null, "subspecies_name": null, "lineage_id": null, "genomovar_id": null},
            "execution": {"status": "incomplete", "stage": "input"},
            "mlst": mlst::not_requested()});
        if args.policy == crate::rescue::V2_POLICY_ID {
            report["schema"] = crate::rescue::REPORT_SCHEMA.into();
            report["policy_id"] = args.policy.clone().into();
            report["trigger"] = Value::Null;
            report["call_basis"] = Value::Null;
            report["reasons"] = json!([]);
            report["legacy_v1_result"] = Value::Null;
            report["rescue_status"] = "incomplete".into();
            report["competitor_evaluation"] = "not_run".into();
            report["evaluation_status"] = "incomplete".into();
            report["counts"] = json!({"unit": "reciprocal_pair", "required": 0, "attempted": 0,
                "completed": 0, "no_alignment": 0, "failed": 0, "not_run": 0});
        }
        if args.policy != V1_POLICY_ID && args.policy != crate::rescue::V2_POLICY_ID {
            report["execution"]["stage"] = "policy".into();
            report["execution"]["error"] = "requested native policy is unavailable; supported policies are conservative-assembly-v1 and conservative-assembly-anim-v2".into();
            exit = 5;
            if let Some(mlst) = &mut mlst {
                report["mlst"] = mlst.not_run("policy");
            }
            results.push(report);
            continue;
        }
        let mut validated = None;
        let result = (|| -> Result<(), (u8, String)> {
            if args.policy == V1_POLICY_ID && args.anim.supplied() {
                report["execution"]["stage"] = "policy".into();
                return Err((
                    5,
                    "ANIm options require conservative-assembly-anim-v2".into(),
                ));
            }
            if let Some(error) = &output_error {
                report["execution"]["stage"] = "anim_output".into();
                return Err((1, format!("ANIm output must be a fresh directory: {error}")));
            }
            if CANCELLED.load(std::sync::atomic::Ordering::SeqCst) {
                return Err((1, "run interrupted".into()));
            }
            if args.input_kind != "assembly" {
                return Err((3, "only assembly FASTA input is supported".into()));
            }
            let bytes: &[u8] = validated.insert(read_input(path, &mut report).map_err(|e| (3, e))?);
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
            let workflow = workflow.as_ref().expect("opened workflow");
            let query_label = path.to_str().ok_or((3, "query path is not UTF-8".into()))?;
            if args.policy == crate::rescue::V2_POLICY_ID {
                workflow.analyse_v2(
                    query_label,
                    bytes,
                    args.threads.into(),
                    &mut report,
                    &v2::V2Context {
                        settings: &args.anim,
                        query_ordinal: index + 1,
                        cancelled: &CANCELLED,
                    },
                )
            } else {
                workflow.analyse(query_label, bytes, args.threads.into(), &mut report)
            }
            .map_err(|e| (1, e))?;
            Ok(())
        })();
        if let Err((code, error)) = &result {
            report["execution"]["status"] = "incomplete".into();
            withhold(&mut report);
            report["execution"]["error"] = error.as_str().into();
            exit = *code;
        }
        // Classification may rebuild the report, so supplementary MLST is attached afterwards.
        report["mlst"] = match &mut mlst {
            Some(mlst) => mlst.report(validated.as_deref()),
            None => mlst::not_requested(),
        };
        if result.is_ok() && report["mlst"]["status"] == "failed" {
            exit = mlst::MLST_FAILED;
        }
        results.push(report);
    }
    if let Some(Err(error)) = mlst.as_ref().map(mlst::Mlst::finish) {
        exit = mlst::MLST_FAILED;
        for report in &mut results {
            mlst::invalidate(report, &error);
        }
    }
    if let Some(workflow) = workflow {
        if results
            .iter()
            .any(|report| report["anim_execution"]["reference_component"].is_object())
        {
            if let Err(error) = workflow.finish_anim(&args.anim) {
                exit = 4;
                for report in &mut results {
                    if report["anim_execution"]["reference_component"].is_object()
                        && report["execution"]["status"] == "complete"
                    {
                        report["execution"] = json!({"status": "incomplete", "stage": "anim_component_recheck", "error": error});
                        withhold(report);
                    }
                }
            }
        }
        if let Err(error) = workflow.finish() {
            exit = 4;
            for report in &mut results {
                if report["execution"]["status"] == "complete" {
                    report["execution"] =
                        json!({"status": "incomplete", "stage": "package_recheck", "error": error});
                    withhold(report);
                }
            }
        }
    }
    if v2::cancel_before_publication(&mut results, &CANCELLED) {
        exit = 1;
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

/// Clear all parent-dependent calls when a wrapper integrity or execution check fails.
pub(crate) fn withhold(report: &mut Value) {
    if let Some(assignment) = report["assignment"].as_object_mut() {
        for key in [
            "species_id",
            "species_name",
            "subspecies_id",
            "subspecies_name",
            "lineage_id",
            "genomovar_id",
        ] {
            assignment.insert(key.into(), Value::Null);
        }
        assignment.insert("status".into(), "not_evaluated".into());
    }
    for key in [
        "species_id",
        "species_name",
        "subspecies_id",
        "subspecies_name",
        "genomovar_id",
    ] {
        if report.get(key).is_some() {
            report[key] = Value::Null;
        }
    }
    if report.get("status").is_some() {
        report["status"] = "unresolved".into();
    }
    if report.get("evaluation_status").is_some() {
        report["evaluation_status"] = "incomplete".into();
    }
    if report.get("rescue_status").is_some() {
        report["rescue_status"] = "incomplete".into();
        report["execution"]["withholding_reason"] = "execution_incomplete".into();
    }
    if report.get("call_basis").is_some() {
        report["call_basis"] = Value::Null;
    }
    for key in ["species", "subspecies", "genomovar"] {
        if report.get(key).is_some() {
            report[key] = Value::Null;
        }
    }
}
static CANCELLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
extern "C" fn cancel_signal(_: libc::c_int) {
    CANCELLED.store(true, std::sync::atomic::Ordering::SeqCst);
}
fn install_cancel_handlers() {
    unsafe {
        libc::signal(
            libc::SIGINT,
            cancel_signal as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGTERM,
            cancel_signal as *const () as libc::sighandler_t,
        );
    }
}
