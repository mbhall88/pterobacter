//! Public command parsing, output and exit codes.
pub mod assembly;
pub mod cache;
pub mod mlst;
pub(crate) mod outputs;
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
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::rescue::{Thresholds, V2_POLICY_ID};

/// Shared threshold flags for native and saved-evidence front ends.
#[derive(Debug, Default, clap::Args)]
pub(crate) struct ThresholdArgs {
    /// Minimum species ANI (%): skani >= this value; ANIm must be strictly greater [default: 96]
    #[arg(short = 's', long, value_name = "PERCENT")]
    species_ani: Option<String>,
    /// Minimum subspecies ANI (%): skani >= this value [default: 98]
    #[arg(short = 'S', long, value_name = "PERCENT")]
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
#[command(
    name = "terobacter",
    version,
    about,
    after_help = "Uses skani first, then ANIm for eligible samples near the species boundary.
ANIm can be slower; use -n/--no-anim to skip it.
ANI thresholds are percentages, e.g. --species-ani 96.5. Require 80 <= species <= subspecies <= 100.
Custom thresholds cannot be combined with --no-anim and mark results custom_unvalidated.
Guide: https://github.com/mbhall88/terobacter/blob/main/docs/assembly.md"
)]
struct Public {
    #[command(flatten)]
    thresholds: ThresholdArgs,
    /// Isolate assembly FASTA files, analysed whole and as supplied
    #[arg(value_name = "ASSEMBLY", required = true, num_args = 1..)]
    assemblies: Vec<PathBuf>,
    /// Authenticated reference package directory
    #[arg(short = 'r', long)]
    reference_package: PathBuf,
    /// Expected SHA-256 of portable.json, obtained from a trusted source
    #[arg(short = 'R', long, value_parser = parse_sha256)]
    expected_sha256: String,
    /// Skip ANIm near the species boundary; use skani only
    #[arg(short = 'n', long)]
    no_anim: bool,
    /// Worker threads for each assembly
    #[arg(short = 't', long, default_value_t = 1, value_parser = clap::value_parser!(u16).range(1..))]
    threads: u16,
    /// Assemblies analysed at the same time, each with its own --threads
    #[arg(short = 'j', long, default_value_t = 1, value_parser = clap::value_parser!(u16).range(1..))]
    jobs: u16,
    /// Write the JSON report to this new file instead of standard output; never replaced
    #[arg(short = 'o', long, value_name = "FILE")]
    output: Option<PathBuf>,
    /// Historical MLST scheme package directory; requests optional supplementary MLST
    #[arg(short = 'm', long, requires = "mlst_expected_sha256")]
    mlst_package: Option<PathBuf>,
    /// Expected SHA-256 of the MLST package manifest.json, obtained from a trusted source
    #[arg(short = 'M', long, value_parser = parse_sha256, requires = "mlst_package")]
    mlst_expected_sha256: Option<String>,
    /// Write report.json, calls.tsv, evidence.tsv and report.md into this new directory
    #[arg(short = 'd', long, value_name = "DIR")]
    report_dir: Option<PathBuf>,
    #[command(flatten)]
    anim: v2::V2Settings,
}
impl Public {
    fn policy_id(&self) -> &'static str {
        if self.no_anim {
            V1_POLICY_ID
        } else {
            V2_POLICY_ID
        }
    }
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
    if args.no_anim
        && (args.thresholds.species_ani.is_some() || args.thresholds.subspecies_ani.is_some())
    {
        eprintln!("error: --no-anim cannot be combined with --species-ani or --subspecies-ani");
        return ExitCode::FAILURE;
    }
    let thresholds = match args.thresholds.for_policy(args.policy_id()) {
        Ok(thresholds) => thresholds,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = outputs::check(&outputs::Paths {
        report: args.output.as_deref(),
        report_dir: args.report_dir.as_deref(),
        anim_output: args.anim.anim_output.as_deref(),
        anim_cache: args.anim.anim_cache.as_deref(),
        packages: [
            Some(args.reference_package.as_path()),
            args.anim.anim_reference_package.as_deref(),
            args.mlst_package.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect(),
        assemblies: args.assemblies.iter().map(PathBuf::as_path).collect(),
    }) {
        eprintln!("error: {error}");
        return ExitCode::from(2);
    }
    if let Some(dir) = &args.report_dir {
        if let Err(error) = fs::create_dir(dir) {
            eprintln!("error: --report-dir must be a new directory: {error}");
            return ExitCode::from(2);
        }
    }
    install_cancel_handlers();
    let output_error = if !args.no_anim {
        args.anim
            .anim_output
            .as_ref()
            .and_then(|p| fs::create_dir(p).err())
            .map(|e| e.to_string())
    } else {
        None
    };
    let mlst = args
        .mlst_package
        .as_deref()
        .zip(args.mlst_expected_sha256.as_deref())
        .map(|(directory, sha256)| mlst::Mlst::new(directory, sha256));
    let batch = Batch {
        args: &args,
        thresholds: &thresholds,
        output_error: output_error.as_deref(),
        workflow: OnceLock::new(),
        mlst: mlst.as_ref(),
    };
    let mut results = Vec::new();
    let mut exit = 0;
    // Reports keep argument order and the exit code follows the last failed input in that
    // order, whatever order the concurrent queries finish in.
    for (report, code) in run_bounded(args.jobs.into(), args.assemblies.len(), |index| {
        batch.analyse(index)
    }) {
        if code != 0 {
            exit = code;
        }
        results.push(report);
    }
    let workflow = batch.workflow.into_inner().and_then(Result::ok);
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
                        && retains_assignment(report)
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
                if retains_assignment(report) {
                    assembly::invalidate(report, "package_recheck", &error);
                }
            }
        }
    }
    if v2::cancel_before_publication(&mut results, &CANCELLED) {
        exit = 1;
    }
    // Publication: a genomovar stands only if its parent is still the report's final call.
    for report in &mut results {
        crate::genomovar::recheck(report);
    }
    for report in &mut results {
        crate::report::annotate(report);
    }
    let rendered = args
        .report_dir
        .as_ref()
        .map(|_| crate::report::render(&results));
    let value = if results.len() == 1 {
        results.remove(0)
    } else {
        Value::Array(results)
    };
    // A failed bundle write must not discard the main JSON output.
    let mut write_failed = false;
    if let (Some(dir), Some(rendered)) = (&args.report_dir, &rendered) {
        if let Err(error) = write_reports(dir, &value, rendered) {
            eprintln!("error writing reports to {}: {error}", dir.display());
            write_failed = true;
        }
    }
    let write_stdout = || -> io::Result<()> {
        let mut out = io::stdout().lock();
        serde_json::to_writer_pretty(&mut out, &value)?;
        writeln!(out)
    };
    let result = match &args.output {
        None => write_stdout(),
        Some(path) => {
            let mut text = serde_json::to_vec_pretty(&value).expect("report serializes");
            text.push(b'\n');
            match outputs::write_new(path, &text) {
                Ok(()) => Ok(()),
                // Keep the results rather than lose them with the file.
                Err(error) => {
                    eprintln!("error writing report to {}: {error}", path.display());
                    if let Err(error) = write_stdout() {
                        eprintln!("error writing report: {error}");
                    }
                    return ExitCode::FAILURE;
                }
            }
        }
    };
    if let Err(error) = result {
        eprintln!("error writing report: {error}");
        return ExitCode::FAILURE;
    }
    if write_failed {
        return ExitCode::FAILURE;
    }
    ExitCode::from(exit)
}

/// Write the JSON report and its renderings into a directory this run created. Each file is
/// created new, so nothing is ever replaced.
pub(crate) fn write_reports(
    dir: &Path,
    json: &Value,
    rendered: &crate::report::Rendered,
) -> io::Result<()> {
    let mut report = serde_json::to_string_pretty(json)?;
    report.push('\n');
    for (name, content) in [
        ("report.json", report.as_str()),
        ("calls.tsv", rendered.calls_tsv.as_str()),
        ("evidence.tsv", rendered.evidence_tsv.as_str()),
        ("report.md", rendered.markdown.as_str()),
    ] {
        outputs::write_new(&dir.join(name), content.as_bytes())?;
    }
    Ok(())
}

/// Shared, read-only state for the queries of one run.
struct Batch<'a> {
    args: &'a Public,
    thresholds: &'a Thresholds,
    output_error: Option<&'a str>,
    /// Opened by the first input that needs it; a failure applies to every later input.
    workflow: OnceLock<Result<assembly::AssemblyWorkflow, String>>,
    mlst: Option<&'a mlst::Mlst>,
}

impl Batch<'_> {
    /// One input's report and exit code, 0 when it completed. Nothing here touches another
    /// input's report.
    fn analyse(&self, index: usize) -> (Value, u8) {
        let args = self.args;
        let path = &args.assemblies[index];
        let mut report = json!({"schema": "terobacter.assembly-call/1",
            "implementation": {"name": env!("CARGO_PKG_NAME"), "version": env!("CARGO_PKG_VERSION")},
            "policy": {"id": args.policy_id()},
            "input": {"kind": "assembly", "file": path.to_string_lossy(), "ordinal": index + 1,
                "sequence_scope": "whole_assembly_as_supplied"},
            "assignment": {"status": "not_evaluated", "species_id": null, "species_name": null,
                "subspecies_id": null, "subspecies_name": null, "lineage_id": null, "genomovar_id": null},
            "execution": {"status": "incomplete", "stage": "input"},
            "mlst": mlst::not_requested()});
        if !args.no_anim {
            report["schema"] = crate::rescue::REPORT_SCHEMA.into();
            for (key, value) in self
                .thresholds
                .policy_value()
                .as_object()
                .expect("policy object")
            {
                report[key] = value.clone();
            }
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
        let mut exit = 0;
        let mut validated = None;
        let result = (|| -> Result<(), (u8, String)> {
            if args.no_anim && args.anim.supplied() {
                report["execution"]["stage"] = "options".into();
                return Err((5, "ANIm options cannot be combined with --no-anim".into()));
            }
            if let Some(error) = self.output_error {
                report["execution"]["stage"] = "anim_output".into();
                return Err((1, format!("ANIm output must be a fresh directory: {error}")));
            }
            if CANCELLED.load(Ordering::SeqCst) {
                return Err((1, "run interrupted".into()));
            }
            let bytes: &[u8] = validated.insert(read_input(path, &mut report).map_err(|e| (3, e))?);
            report["execution"]["stage"] = "package".into();
            let workflow = self
                .workflow
                .get_or_init(|| {
                    assembly::AssemblyWorkflow::open(&args.reference_package, &args.expected_sha256)
                })
                .as_ref()
                .map_err(|e| (4, e.clone()))?;
            let query_label = path.to_str().ok_or((3, "query path is not UTF-8".into()))?;
            if !args.no_anim {
                workflow.analyse_v2(
                    query_label,
                    bytes,
                    args.threads.into(),
                    &mut report,
                    &v2::V2Context {
                        settings: &args.anim,
                        query_ordinal: index + 1,
                        cancelled: &CANCELLED,
                        thresholds: self.thresholds,
                    },
                )
            } else {
                workflow.analyse(query_label, bytes, args.threads.into(), &mut report)
            }
            .map_err(|e| (1, e))?;
            Ok(())
        })();
        if let Err((code, error)) = &result {
            exit = record_failure(&mut report, *code, error.clone());
        }
        // Classification may rebuild the report, so supplementary MLST is attached afterwards.
        report["mlst"] = match self.mlst {
            Some(mlst) => mlst.report(validated.as_deref()),
            None => mlst::not_requested(),
        };
        if result.is_ok() && report["mlst"]["status"] == "failed" {
            exit = mlst::MLST_FAILED;
        }
        (report, exit)
    }
}

/// Run `task` for every index below `count` on at most `jobs` threads. Results keep index
/// order, independent of completion order.
fn run_bounded<T: Send>(jobs: usize, count: usize, task: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let next = AtomicUsize::new(0);
    let slots: Vec<Mutex<Option<T>>> = (0..count).map(|_| Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..jobs.clamp(1, count.max(1)) {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::SeqCst);
                if index >= count {
                    break;
                }
                let value = task(index);
                *slots[index].lock().expect("result slot") = Some(value);
            });
        }
    });
    slots
        .into_iter()
        .map(|slot| {
            slot.into_inner()
                .expect("result slot")
                .expect("every index ran")
        })
        .collect()
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
    for key in ["species", "subspecies"] {
        if report.get(key).is_some() {
            report[key] = Value::Null;
        }
    }
}
static CANCELLED: AtomicBool = AtomicBool::new(false);
/// The first SIGINT or SIGTERM stops the run at its next check and still publishes the
/// withheld reports. A second signal of the same kind terminates at once.
extern "C" fn cancel_signal(signal: libc::c_int) {
    CANCELLED.store(true, Ordering::SeqCst);
    unsafe {
        libc::signal(signal, libc::SIG_DFL);
    }
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

fn record_failure(report: &mut Value, code: u8, error: String) -> u8 {
    report["execution"]["status"] = "incomplete".into();
    if report["execution"]["stage"] != "genomovar" {
        withhold(report);
    }
    report["execution"]["error"] = error.into();
    code
}

/// Final integrity checks apply even when a supplementary stage failed.
fn retains_assignment(report: &Value) -> bool {
    report["execution"]["status"] == "complete"
        || report["assignment"]["species_id"].is_string()
        || report["species_id"].is_string()
}

#[cfg(test)]
mod supplementary_integrity_tests {
    use super::*;

    #[test]
    fn incomplete_supplement_still_requires_final_parent_integrity_checks() {
        for mut report in [
            json!({"assignment": {"status": "assigned", "species_id": "parent", "subspecies_id": "child"}}),
            json!({"status": "assigned", "species_id": "parent", "subspecies_id": "child"}),
        ] {
            let error =
                assembly::attach_result(&mut report, Err("supplement failed".into())).unwrap_err();
            assert_eq!(record_failure(&mut report, 1, error), 1);
            assert_eq!(report["genomovar"]["status"], "failed");
            assert!(retains_assignment(&report));
            assembly::invalidate(&mut report, "package_recheck", "changed package");
            assert!(!retains_assignment(&report));
            assert_eq!(report["species_id"], Value::Null);
            assert_eq!(report["assignment"]["species_id"], Value::Null);
        }
    }
}

#[cfg(test)]
mod public_option_tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn every_public_option_has_a_unique_short_form() {
        let command = Public::command();
        command.clone().debug_assert();
        let mut seen = BTreeSet::new();
        for arg in command.get_arguments().filter(|a| a.get_long().is_some()) {
            let short = arg
                .get_short()
                .unwrap_or_else(|| panic!("missing short form: {}", arg.get_id()));
            assert!(seen.insert(short), "duplicate -{short}");
        }
    }

    #[test]
    fn short_options_preserve_every_long_option_value() {
        let pin = "1".repeat(64);
        let mlst_pin = "2".repeat(64);
        let anim_pin = "3".repeat(64);
        let options = [
            ("--species-ani", "-s", "95.5"),
            ("--subspecies-ani", "-S", "97.5"),
            ("--reference-package", "-r", "refs"),
            ("--expected-sha256", "-R", pin.as_str()),
            ("--threads", "-t", "2"),
            ("--jobs", "-j", "3"),
            ("--output", "-o", "calls.json"),
            ("--mlst-package", "-m", "mlst"),
            ("--mlst-expected-sha256", "-M", mlst_pin.as_str()),
            ("--report-dir", "-d", "reports"),
            ("--anim-reference-package", "-a", "fastas"),
            ("--expected-anim-sha256", "-A", anim_pin.as_str()),
            ("--anim-output", "-e", "evidence"),
            ("--anim-cache", "-c", "cache"),
            ("--anim-pair-seconds", "-T", "12.5"),
            ("--anim-query-seconds", "-Q", "120.5"),
            ("--anim-memory-mib", "-L", "512"),
        ];
        let mut long = vec!["tero", "query.fa"];
        let mut short = long.clone();
        for (l, s, value) in options {
            long.extend([l, value]);
            short.extend([s, value]);
        }
        let long = Public::try_parse_from(long).unwrap();
        let short = Public::try_parse_from(short).unwrap();
        assert_eq!(format!("{long:?}"), format!("{short:?}"));
        assert!(!short.no_anim);
        assert_eq!(short.anim.anim_pair_seconds.unwrap().as_secs_f64(), 12.5);
        assert_eq!(short.anim.anim_query_seconds.unwrap().as_secs_f64(), 120.5);
    }
}
