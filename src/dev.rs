//! Development binary: evaluation-only operations (saved-evidence classification, reference
//! package authentication, measurement and artifact verification). Compiled only with the `dev`
//! feature, which the module declaration in `lib.rs` gates, so a default build or install never
//! contains any of it. Failure produces no decision on stdout.
use crate::cli::Directory;
use crate::decision::{decide, decide_bound, Policy, V1_POLICY_ID};
use crate::evidence::{sha256_hex, SavedEvidence};
use crate::panel::Panel;
use crate::reference::{
    is_sha256, verify_package, PackageError, PackageErrorKind, RegistryPins, VerifiedPackage,
};
use crate::rescue::{AnimRecords, BoundAnim, RescueInput, V2_POLICY_ID};
use crate::skani::{SearchSettings, SeedingMode, SkaniDatabase};
use clap::{Args, Parser, Subcommand};
use serde_json::{json, Value};
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "pterobacter-dev",
    version,
    about = "Evaluation-only operations. Not part of the public command"
)]
struct Dev {
    #[command(subcommand)]
    command: DevCommand,
}
#[derive(Debug, Subcommand)]
enum DevCommand {
    /// Measure a reciprocal ANIm pair with the approved maintained MUMmer4 method
    AnimPair(Box<AnimPairArgs>),
    /// Parse filtered delta evidence using supplied whole-input length maps
    AnimParse {
        #[arg(long)]
        filtered_delta: PathBuf,
        #[arg(long)]
        reference_lengths: PathBuf,
        #[arg(long)]
        query_lengths: PathBuf,
    },
    /// Decide species/subspecies from saved 0.2.0 evidence; no new measurements
    SavedEvidence {
        #[command(flatten)]
        inputs: Inputs,
        /// Calling policy
        #[arg(long, default_value = V1_POLICY_ID, value_parser = [V1_POLICY_ID, V2_POLICY_ID])]
        policy: String,
        /// Bound reciprocal ANIm diagnostic envelope (v1 only)
        #[arg(long, requires_all = ["diagnostics_sha256", "reference_inventory"])]
        diagnostics: Option<PathBuf>,
        #[arg(long, requires = "diagnostics")]
        diagnostics_sha256: Option<String>,
        #[command(flatten)]
        v2: V2Inputs,
        #[command(flatten)]
        package: PackageInputs,
        /// Also write report.json, calls.tsv, evidence.tsv and report.md to this new directory
        #[arg(long, value_name = "NEW_DIRECTORY")]
        report_dir: Option<PathBuf>,
    },
    /// Verify a reference package directory and print its identity and components as JSON
    ReferencePackage {
        /// Directory holding portable.json and the files it lists
        directory: PathBuf,
        /// SHA-256 that the package's portable.json must have
        #[arg(long, value_name = "SHA256", value_parser = parse_sha256)]
        expected_sha256: String,
        /// SHA-256 that the named registry table must have, if it is to be pinned
        #[arg(long, value_name = "SHA256", value_parser = parse_sha256)]
        named_registry_sha256: Option<String>,
        /// SHA-256 that the lineage registry table must have, if it is to be pinned
        #[arg(long, value_name = "SHA256", value_parser = parse_sha256)]
        lineage_registry_sha256: Option<String>,
    },
    /// Dry v2 rescue plan: trigger, nearest parent, witnesses and the ANIm pairs still
    /// required. Reads saved evidence and records only; runs no alignment
    RescuePlan {
        #[command(flatten)]
        inputs: Inputs,
        #[command(flatten)]
        v2: V2Inputs,
        #[command(flatten)]
        package: PackageInputs,
    },
    /// Reassess saved genomovar member evidence after development-only masks or group removals
    GenomovarDiagnostic(Box<GenomovarDiagnosticArgs>),
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

#[derive(Debug, Args)]
struct GenomovarDiagnosticArgs {
    /// Completed assembly report produced by pterobacter
    #[arg(long)]
    report: PathBuf,
    /// SHA-256 of the exact report bytes
    #[arg(long, value_name = "SHA256", value_parser = parse_sha256)]
    expected_report_sha256: String,
    /// Authenticated reference package directory used by the report
    #[arg(long, value_name = "DIR")]
    reference_package: PathBuf,
    /// SHA-256 that the package's portable.json must have
    #[arg(long, value_name = "SHA256", value_parser = parse_sha256)]
    expected_sha256: String,
    /// Member reference ID to mask; repeat for multiple members
    #[arg(long = "mask-member")]
    masked_members: Vec<String>,
    /// Entire genomovar group ID to remove; repeat for multiple groups
    #[arg(long = "remove-group")]
    removed_groups: Vec<String>,
    /// Omit a saved member row while retaining its required definition membership
    #[arg(long = "exclude-evidence-row")]
    excluded_evidence_rows: Vec<String>,
    /// Primary-mask setting used for this diagnostic
    #[arg(long, default_value = "unmasked", value_parser = ["unmasked", "self_alias", "clone_block"])]
    setting: String,
    /// Matching primary-reference mask: one ID per line; an explicitly pinned empty file is valid
    #[arg(long, requires = "exclude_references_sha256")]
    exclude_references: Option<PathBuf>,
    /// SHA-256 of the matching primary-mask file
    #[arg(long, value_parser = parse_sha256, requires = "exclude_references")]
    exclude_references_sha256: Option<String>,
    /// Additional bound warning envelope for the recomputed parent
    #[arg(long, requires = "problem_warnings_sha256")]
    problem_warnings: Option<PathBuf>,
    #[arg(long, value_parser = parse_sha256, requires = "problem_warnings")]
    problem_warnings_sha256: Option<String>,
    /// Bound reciprocal ANIm records for a recomputed v2 parent
    #[arg(long, requires = "anim_method_profile")]
    anim_records: Vec<PathBuf>,
    #[arg(long, requires = "anim_records")]
    anim_method_profile: Option<String>,
}
#[derive(Debug, Args)]
struct Inputs {
    /// Saved evidence JSON, schema 2.0.0-dev-evidence
    evidence: PathBuf,
    /// Optional expected SHA-256 of the exact saved evidence bytes
    #[arg(long)]
    evidence_sha256: Option<String>,
    /// Primary inventory from a checked reference package; required for v1 method diagnostics
    /// and masks. v2 and the rescue plan take it or --reference-package
    #[arg(long)]
    reference_inventory: Option<PathBuf>,
    #[arg(long, requires = "reference_inventory")]
    reference_inventory_sha256: Option<String>,
    /// Bound external problem-warning envelope
    #[arg(long, requires = "problem_warnings_sha256")]
    problem_warnings: Option<PathBuf>,
    #[arg(long, requires = "problem_warnings")]
    problem_warnings_sha256: Option<String>,
}
#[derive(Debug, Args)]
struct V2Inputs {
    #[command(flatten)]
    thresholds: crate::cli::ThresholdArgs,
    /// Saved ANIm pair records: JSON Lines, one pretty-printed `.json` record, or a directory
    /// of `.jsonl` files. Only records for this query and for pairs the dry plan requests are
    /// bound
    #[arg(long, requires = "anim_method_profile")]
    anim_records: Vec<PathBuf>,
    /// The ANIm method profile every bound record must carry
    #[arg(long, requires = "anim_records")]
    anim_method_profile: Option<String>,
    /// Primary-reference mask: one reference ID per line, removed from the skani evidence and
    /// from the ANIm competitor set
    #[arg(long)]
    exclude_references: Option<PathBuf>,
}
/// A reference package to verify and bind the evidence to. Under v2 it also supplies the
/// primary panel.
#[derive(Debug, Args)]
struct PackageInputs {
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
    /// SHA-256 that the named registry table must have, if it is to be pinned
    #[arg(
        long,
        value_name = "SHA256",
        value_parser = parse_sha256,
        requires = "reference_package"
    )]
    named_registry_sha256: Option<String>,
    /// SHA-256 that the lineage registry table must have, if it is to be pinned
    #[arg(
        long,
        value_name = "SHA256",
        value_parser = parse_sha256,
        requires = "reference_package"
    )]
    lineage_registry_sha256: Option<String>,
}

impl PackageInputs {
    /// Verify the named package, or return `None` when no package was given.
    fn verify(&self) -> Result<Option<VerifiedPackage>, Failure> {
        let Some((directory, expected)) = self
            .reference_package
            .as_ref()
            .zip(self.expected_sha256.as_ref())
        else {
            return Ok(None);
        };
        let pins = RegistryPins {
            named_registry_sha256: self.named_registry_sha256.clone(),
            lineage_registry_sha256: self.lineage_registry_sha256.clone(),
        };
        Ok(Some(verify_package(
            &Directory(directory.clone()),
            expected,
            &pins,
        )?))
    }
}

fn parse_sha256(text: &str) -> Result<String, String> {
    if is_sha256(text) {
        Ok(text.to_string())
    } else {
        Err("expected 64 lowercase hexadecimal characters".to_string())
    }
}

#[derive(Debug, clap::Args)]
struct AnimPairArgs {
    #[arg(long)]
    query: PathBuf,
    #[arg(long)]
    reference: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    query_id: Option<String>,
    #[arg(long)]
    reference_id: Option<String>,
    #[arg(long, default_value = "primary-whole-assembly")]
    reference_component: String,
    #[arg(long)]
    reference_concept_id: Vec<String>,
    #[arg(long)]
    expect_query_sha256: Option<String>,
    #[arg(long)]
    expect_reference_sha256: Option<String>,
    /// Opt-in reciprocal pair seconds; omitted or 0 means unlimited
    #[arg(long, value_parser = crate::cli::v2::parse_seconds)]
    timeout_seconds: Option<std::time::Duration>,
    /// Opt-in worker MiB; omitted or 0 means unlimited
    #[arg(long, value_parser = crate::cli::v2::parse_memory)]
    memory_mib: Option<u64>,
}

const EXIT_FAILURE: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_MISSING: u8 = 3;
const EXIT_ALTERED: u8 = 4;
const EXIT_TRUNCATED: u8 = 5;
const EXIT_INCOMPATIBLE: u8 = 6;
const EXIT_INCONSISTENT: u8 = 7;
const EXIT_UNRESOLVED: u8 = 8;
const EXIT_UNEXPECTED: u8 = 9;
const EXIT_UNLISTED: u8 = 10;

/// Why a command printed no result, and the exit code that says so.
struct Failure {
    code: u8,
    message: String,
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Failure {
            code: EXIT_FAILURE,
            message,
        }
    }
}

impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        message.to_string().into()
    }
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
            PackageErrorKind::Inconsistent => EXIT_INCONSISTENT,
            PackageErrorKind::Unresolved => EXIT_UNRESOLVED,
            PackageErrorKind::Unlisted => EXIT_UNLISTED,
        };
        Failure {
            code,
            message: error.to_string(),
        }
    }
}

/// Run the development binary.
pub fn run() -> ExitCode {
    if let Some(status) = crate::anim::run_worker_if_requested() {
        return status;
    }
    match Dev::parse().command {
        DevCommand::AnimParse {
            filtered_delta,
            reference_lengths,
            query_lengths,
        } => {
            let result = (|| -> Result<_, crate::anim::AnimError> {
                let refs = serde_json::from_slice(&std::fs::read(reference_lengths)?)?;
                let queries = serde_json::from_slice(&std::fs::read(query_lengths)?)?;
                crate::anim::parse_delta(&std::fs::read_to_string(filtered_delta)?, &refs, &queries)
            })();
            match result {
                Ok(metrics) => {
                    println!("{}", serde_json::to_string_pretty(&metrics).unwrap());
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("error: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        command @ (DevCommand::SavedEvidence { .. }
        | DevCommand::ReferencePackage { .. }
        | DevCommand::SkaniSearch { .. }
        | DevCommand::RescuePlan { .. }
        | DevCommand::GenomovarDiagnostic(_)) => match execute(command) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(failure) => {
                eprintln!("error: {}", failure.message);
                ExitCode::from(failure.code)
            }
        },
        DevCommand::AnimPair(args) => {
            let AnimPairArgs {
                query,
                reference,
                output,
                query_id,
                reference_id,
                reference_component,
                reference_concept_id,
                expect_query_sha256,
                expect_reference_sha256,
                timeout_seconds,
                memory_mib,
            } = *args;
            let timeout = timeout_seconds.filter(|time| !time.is_zero());
            let memory_mib = memory_mib.filter(|mib| *mib != 0);
            let execution = crate::anim::limits::inherited().and_then(|inherited| {
                crate::anim::limits::Execution::new(timeout, None, memory_mib, inherited)
            });
            let execution = match execution {
                Ok(value) => value,
                Err(error) => {
                    eprintln!("error: {error}");
                    return ExitCode::FAILURE;
                }
            };
            let request = crate::anim::PairRequest {
                query,
                reference,
                output,
                query_id,
                reference_id,
                reference_component,
                reference_concept_ids: reference_concept_id,
                expect_query_sha256,
                expect_reference_sha256,
                timeout,
                memory_mib,
                execution: Some(execution),
            };
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
            match crate::anim::measure_pair(&request, &CANCELLED) {
                Ok(report) => {
                    println!("{}", serde_json::to_string_pretty(&report).unwrap());
                    if report.status == "complete" {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::FAILURE
                    }
                }
                Err(error) => {
                    eprintln!("error: {error}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}
static CANCELLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
extern "C" fn cancel_signal(signal: libc::c_int) {
    CANCELLED.store(true, std::sync::atomic::Ordering::SeqCst);
    unsafe {
        libc::signal(signal, libc::SIG_DFL);
    }
}

type Pins = Vec<(PathBuf, String)>;

fn reference_package(
    directory: &Path,
    expected_sha256: &str,
    pins: &RegistryPins,
) -> Result<String, Failure> {
    let package: VerifiedPackage =
        verify_package(&Directory(directory.to_path_buf()), expected_sha256, pins)?;
    Ok(serde_json::to_string_pretty(&package.to_value()).expect("a JSON value always serialises"))
}

fn read_bytes(path: &Path, expected: Option<&str>, pins: &mut Pins) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let hash = sha256_hex(&bytes);
    if expected.is_some_and(|digest| digest != hash) {
        return Err(format!("{}: SHA-256 mismatch", path.display()));
    }
    pins.push((path.to_path_buf(), hash));
    Ok(bytes)
}
fn read(path: &Path, expected: Option<&str>, pins: &mut Pins) -> Result<String, String> {
    let fail = |error: &dyn Display| format!("{}: {error}", path.display());
    String::from_utf8(read_bytes(path, expected, pins)?).map_err(|e| fail(&e))
}
fn verify_artifact(path: &Path, expected: &str, pins: &mut Pins) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let actual = sha256_hex(&bytes);
    if actual != expected {
        return Err(format!("{}: artifact SHA-256 mismatch", path.display()));
    }
    pins.push((path.to_path_buf(), actual));
    Ok(())
}
fn verify_artifacts(evidence: &SavedEvidence, pins: &mut Pins) -> Result<(), String> {
    if let Some(checks) = evidence.scope()["checks"].as_array() {
        for check in checks {
            let path = Path::new(
                check["artifact_path"]
                    .as_str()
                    .expect("validated scope path"),
            );
            if !path.is_absolute() {
                return Err("scope artifacts must use absolute paths".into());
            }
            verify_artifact(
                path,
                check["sha256"].as_str().expect("validated scope hash"),
                pins,
            )?;
        }
    }
    for item in evidence
        .diagnostics()
        .iter()
        .chain(evidence.problem_warnings())
    {
        for pin in item["artifacts"]
            .as_array()
            .expect("validated artifact list")
        {
            verify_artifact(
                Path::new(pin["path"].as_str().expect("validated artifact path")),
                pin["sha256"].as_str().expect("validated artifact hash"),
                pins,
            )?;
        }
    }
    Ok(())
}

/// Saved evidence with its optional annotations, verified against the files they pin.
fn load(
    inputs: &Inputs,
    diagnostics: Option<(&Path, Option<&str>)>,
    pins: &mut Pins,
) -> Result<SavedEvidence, String> {
    let text = read(&inputs.evidence, inputs.evidence_sha256.as_deref(), pins)?;
    let saved = SavedEvidence::from_json(&text)
        .map_err(|e| format!("{}: {e}", inputs.evidence.display()))?;
    let method = diagnostics
        .map(|(path, hash)| read(path, hash, pins))
        .transpose()?;
    let inventory = match (&inputs.reference_inventory, &method) {
        (Some(path), Some(_)) => Some(read(
            path,
            inputs.reference_inventory_sha256.as_deref(),
            pins,
        )?),
        _ => None,
    };
    let warning = inputs
        .problem_warnings
        .as_ref()
        .map(|path| read(path, inputs.problem_warnings_sha256.as_deref(), pins))
        .transpose()?;
    let saved = saved
        .with_annotations(method.as_deref(), warning.as_deref(), inventory.as_deref())
        .map_err(|e| e.to_string())?;
    verify_artifacts(&saved, pins)?;
    Ok(saved)
}

/// Every file named, plus the `.jsonl` files directly inside a named directory, in name order.
fn record_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for path in paths {
        if path.is_dir() {
            let mut found: Vec<_> = std::fs::read_dir(path)
                .map_err(|e| format!("{}: {e}", path.display()))?
                .map(|entry| entry.map(|e| e.path()))
                .collect::<Result<_, _>>()
                .map_err(|e| format!("{}: {e}", path.display()))?;
            found.retain(|p| p.is_file() && p.extension().is_some_and(|x| x == "jsonl"));
            found.sort();
            files.extend(found);
        } else {
            files.push(path.clone());
        }
    }
    Ok(files)
}

/// A primary-reference mask file: one reference ID per line, blank lines ignored.
fn read_mask(path: &Path, pins: &mut Pins) -> Result<Vec<String>, String> {
    Ok(read(path, None, pins)?
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect())
}

/// The v1 decision on saved evidence with masked references removed from the rows and the
/// panel counts, exactly as v2 masks the evidence behind its `legacy_v1_result`. A package
/// that supplied the panel is checked against the whole, unmasked evidence and returned so the
/// result can record it.
fn masked_v1(
    inputs: &Inputs,
    package: &PackageInputs,
    mask: &Path,
    pins: &mut Pins,
) -> Result<(Value, Option<VerifiedPackage>), Failure> {
    let saved = load(inputs, None, pins)?;
    let (panel, verified) = primary_panel(inputs, package, &saved, pins, "under v1, a mask")?;
    let (masked, assessed, excluded) = panel
        .mask(&saved, &read_mask(mask, pins)?)
        .map_err(|e| e.to_string())?;
    let mut value = decide(&masked, &Policy::v1()).to_value().clone();
    value["input"]["primary_inventory_sha256"] = panel.source_sha256().into();
    value["input"]["mask"] = json!({"excluded_reference_ids": excluded,
        "panel_reference_count": panel.references().len(),
        "assessed_panel_reference_count": assessed.references().len()});
    Ok((value, verified))
}

/// The whole primary panel, from exactly one of a pinned inventory file or an authenticated
/// package, checked against `saved`. `needed_by` names what requires it in the refusal.
fn primary_panel(
    inputs: &Inputs,
    package: &PackageInputs,
    saved: &SavedEvidence,
    pins: &mut Pins,
    needed_by: &str,
) -> Result<(Panel, Option<VerifiedPackage>), Failure> {
    match (&inputs.reference_inventory, &package.reference_package) {
        (Some(_), Some(_)) => {
            Err("give either --reference-inventory or --reference-package, not both".into())
        }
        (None, None) => Err(format!(
            "{needed_by} needs the whole primary panel: give --reference-inventory \
             or --reference-package"
        )
        .into()),
        (Some(inventory), None) => {
            let text = read(
                inventory,
                inputs.reference_inventory_sha256.as_deref(),
                pins,
            )?;
            let panel = Panel::from_json(&text, saved).map_err(|e| e.to_string())?;
            Ok((panel, None))
        }
        (None, Some(_)) => {
            let verified = package.verify()?.expect("package given");
            // Checked here first so evidence bound to another package keeps its exit code.
            verified.check_evidence(saved)?;
            let panel = Panel::from_package(&verified, saved).map_err(|e| e.to_string())?;
            Ok((panel, Some(verified)))
        }
    }
}

/// v2 inputs: evidence, panel, mask and any saved ANIm records, bound by the dry-plan rule.
/// The panel comes from a primary inventory or from an authenticated package, which is then
/// returned so the result can record it.
fn rescue_inputs(
    inputs: &Inputs,
    v2: &V2Inputs,
    package: &PackageInputs,
    pins: &mut Pins,
) -> Result<(RescueInput, BoundAnim, Value, Option<VerifiedPackage>), Failure> {
    let thresholds = v2.thresholds.for_policy(V2_POLICY_ID)?;
    let saved = load(inputs, None, pins)?;
    let (panel, verified) = primary_panel(inputs, package, &saved, pins, V2_POLICY_ID)?;
    let mask = match &v2.exclude_references {
        Some(path) => read_mask(path, pins)?,
        None => vec![],
    };
    let rescue = RescueInput::new(saved, panel, &mask, thresholds).map_err(|e| e.to_string())?;
    let mut records = AnimRecords::default();
    let mut files = Vec::new();
    for path in record_files(&v2.anim_records)? {
        let text = read(&path, None, pins)?;
        // A `.json` file holds one record as the per-pair operation writes it.
        let parsed = if path.extension().is_some_and(|x| x == "json") {
            AnimRecords::from_json(&text)
        } else {
            AnimRecords::from_jsonl(&text)
        }
        .map_err(|e| format!("{}: {e}", path.display()))?;
        files.push(
            json!({"file_name": path.file_name().map(|n| n.to_string_lossy()),
            "sha256": pins.last().expect("pinned").1, "records": parsed.len()}),
        );
        records.extend(parsed);
    }
    let bound = match &v2.anim_method_profile {
        Some(profile) => rescue
            .bind_to_plan(&records, profile)
            .map_err(|e| e.to_string())?,
        None => BoundAnim::none(),
    };
    Ok((rescue, bound, Value::Array(files), verified))
}

const UNBOUND: &str =
    "input_and_artifact_hashes_checked; package_authentication_requires_checked_inventory";
const BOUND: &str = "input_and_artifact_hashes_checked; reference_package_verified";

/// Record the package that supplied a v2 panel, as v1 bound decisions do, and return the
/// verification label that applies.
fn record_package(value: &mut Value, package: Option<VerifiedPackage>) -> &'static str {
    match package {
        Some(package) => {
            value["reference_package"] = package.to_value();
            BOUND
        }
        None => UNBOUND,
    }
}

fn execute(command: DevCommand) -> Result<String, Failure> {
    // The report directory is checked and created before any work, so a collision changes
    // nothing. The shared output checks refuse a destination inside the reference package,
    // however it is spelled, because the package's inventory rejects any unlisted file.
    let report_dir = match &command {
        DevCommand::SavedEvidence {
            report_dir: Some(dir),
            package,
            ..
        } => {
            crate::cli::outputs::check(&crate::cli::outputs::Paths {
                report: None,
                report_dir: Some(dir),
                anim_output: None,
                anim_cache: None,
                packages: package
                    .reference_package
                    .iter()
                    .map(PathBuf::as_path)
                    .collect(),
                assemblies: vec![],
            })
            .map_err(|message| Failure {
                code: EXIT_USAGE,
                message,
            })?;
            std::fs::create_dir(dir).map_err(|error| Failure {
                code: EXIT_USAGE,
                message: format!("--report-dir must be a new directory: {error}"),
            })?;
            Some(dir.clone())
        }
        _ => None,
    };
    let result = match evaluate(command) {
        Ok(Evaluated::Printed(output)) => return Ok(output),
        Ok(Evaluated::Report(result)) => result,
        Err(failure) => {
            // Nothing has been written yet, so this removes only the directory made above,
            // and the same command can be run again.
            if let Some(dir) = &report_dir {
                let _ = std::fs::remove_dir(dir);
            }
            return Err(failure);
        }
    };
    let output = serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?;
    if let Some(dir) = report_dir {
        let rendered = crate::report::render(std::slice::from_ref(&result));
        if let Err(error) = crate::cli::write_reports(&dir, &result, &rendered) {
            // As in the public command, a failed report write keeps the main JSON output.
            println!("{output}");
            return Err(format!("error writing reports to {}: {error}", dir.display()).into());
        }
    }
    Ok(output)
}

/// What a development command produced: output it already formatted, or a decision report
/// whose pinned inputs have been rechecked.
enum Evaluated {
    Printed(String),
    Report(Value),
}

fn evaluate(command: DevCommand) -> Result<Evaluated, Failure> {
    let mut pins = Vec::new();
    let mut verification = UNBOUND;
    let annotate_report = matches!(command, DevCommand::SavedEvidence { .. });
    let mut result = match command {
        DevCommand::AnimPair(_) | DevCommand::AnimParse { .. } => {
            return Err("ANIm command must be dispatched through run".into());
        }
        DevCommand::GenomovarDiagnostic(args) => {
            return genomovar_diagnostic(&args).map(Evaluated::Printed);
        }
        DevCommand::SavedEvidence {
            inputs,
            policy,
            diagnostics,
            diagnostics_sha256,
            v2,
            package,
            ..
        } if policy == V1_POLICY_ID => {
            v2.thresholds.for_policy(V1_POLICY_ID)?;
            if !v2.anim_records.is_empty() {
                return Err(format!(
                    "--anim-records needs --policy {V2_POLICY_ID}; \
                     v1 takes method diagnostics through --diagnostics"
                )
                .into());
            }
            if let Some(mask) = &v2.exclude_references {
                if diagnostics.is_some() {
                    return Err(
                        "under v1, --exclude-references cannot be combined with --diagnostics"
                            .into(),
                    );
                }
                let (mut value, verified) = masked_v1(&inputs, &package, mask, &mut pins)?;
                verification = record_package(&mut value, verified);
                value
            } else if inputs.reference_inventory.is_some() && diagnostics.is_none() {
                return Err(
                    "under v1, --reference-inventory pairs with --diagnostics or \
                     --exclude-references"
                        .into(),
                );
            } else {
                let diagnostics = diagnostics
                    .as_deref()
                    .map(|path| (path, diagnostics_sha256.as_deref()));
                let saved = load(&inputs, diagnostics, &mut pins)?;
                match package.verify()? {
                    Some(package) => {
                        verification = BOUND;
                        decide_bound(&saved, &package, &Policy::v1())?
                            .to_value()
                            .clone()
                    }
                    None => decide(&saved, &Policy::v1()).to_value().clone(),
                }
            }
        }
        DevCommand::SavedEvidence {
            inputs,
            diagnostics,
            v2,
            package,
            ..
        } => {
            if diagnostics.is_some() {
                return Err(format!(
                    "{V2_POLICY_ID} reads ANIm through --anim-records, not v1 --diagnostics"
                )
                .into());
            }
            let (rescue, bound, files, verified) =
                rescue_inputs(&inputs, &v2, &package, &mut pins)?;
            let mut value = rescue
                .decide(&bound)
                .map_err(|e| e.to_string())?
                .to_value()
                .clone();
            value["input"]["anim_record_files"] = files;
            verification = record_package(&mut value, verified);
            value
        }
        DevCommand::ReferencePackage {
            directory,
            expected_sha256,
            named_registry_sha256,
            lineage_registry_sha256,
        } => {
            return reference_package(
                &directory,
                &expected_sha256,
                &RegistryPins {
                    named_registry_sha256,
                    lineage_registry_sha256,
                },
            )
            .map(Evaluated::Printed);
        }
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
            return SkaniDatabase::open(&database)
                .and_then(|db| db.search(&query, settings, threads, mode))
                .map_err(|e| e.to_string())
                .and_then(|report| serde_json::to_string_pretty(&report).map_err(|e| e.to_string()))
                .map(Evaluated::Printed)
                .map_err(Failure::from);
        }
        DevCommand::RescuePlan {
            inputs,
            v2,
            package,
        } => {
            let (rescue, bound, files, verified) =
                rescue_inputs(&inputs, &v2, &package, &mut pins)?;
            let mut value = rescue
                .plan(&bound)
                .map_err(|e| e.to_string())?
                .to_value()
                .clone();
            value["input"]["anim_record_files"] = files;
            verification = record_package(&mut value, verified);
            value
        }
    };
    result["input"]["verification"] = Value::String(verification.into());
    if annotate_report {
        crate::report::annotate(&mut result);
    }
    // Recheck the originally pinned bytes before a successful result can leave this wrapper.
    for (path, hash) in &pins {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if sha256_hex(&bytes) != *hash {
            return Err(format!("{}: input changed during decision", path.display()).into());
        }
    }
    Ok(Evaluated::Report(result))
}

fn genomovar_diagnostic(args: &GenomovarDiagnosticArgs) -> Result<String, Failure> {
    let report_path = &args.report;
    let expected_report_sha256 = args.expected_report_sha256.as_str();
    let package_path = &args.reference_package;
    let expected_package_sha256 = args.expected_sha256.as_str();
    let mut pins = Vec::new();
    let report_bytes = std::fs::read(report_path)
        .map_err(|error| format!("{}: {error}", report_path.display()))?;
    let report_sha256 = sha256_hex(&report_bytes);
    if report_sha256 != expected_report_sha256 {
        return Err(format!(
            "report SHA-256 is {report_sha256}, not the expected {expected_report_sha256}"
        )
        .into());
    }
    pins.push((report_path.clone(), report_sha256.clone()));
    let report: Value = serde_json::from_slice(&report_bytes)
        .map_err(|error| format!("{}: invalid report JSON: {error}", report_path.display()))?;
    let package = verify_package(
        &Directory(package_path.to_path_buf()),
        expected_package_sha256,
        &RegistryPins::default(),
    )?;
    let definition =
        crate::genomovar::Definition::from_package(&package).map_err(|error| error.to_string())?;
    let component = package
        .component(crate::genomovar::COMPONENT_ID)
        .ok_or("verified package has no genomovar component")?;
    let section = &report["genomovar"];
    let search = &section["search"];
    let fail = |message: &str| -> Failure { message.to_string().into() };

    if report["execution"]["status"] != "complete"
        || report["execution"]["stage"] != "genomovar"
        || report["input"]["kind"] != "assembly"
        || report["input"]["sequence_scope"] != crate::genomovar::QUERY_SCOPE
        || report["input"]["verification"] != "query_bytes_hashed; reference_package_verified"
        || !report["input"]["query_sha256"]
            .as_str()
            .is_some_and(is_sha256)
        || report["input"]["reference_package_sha256"] != expected_package_sha256
        || report["reference_package"]["package"]["manifest_sha256"] != expected_package_sha256
    {
        return Err(fail("report is not a completed assembly genomovar report"));
    }
    if search["status"] != "completed"
        || search["component_id"] != crate::genomovar::COMPONENT_ID
        || search["input_kind"] != "assembly"
        || search["query_scope"] != crate::genomovar::QUERY_SCOPE
        || search["diagnostic_exclusion"] != false
        || search["adapter_verified"] != true
        || search["query_sha256"] != report["input"]["query_sha256"]
        || search["reference_package_sha256"] != expected_package_sha256
        || search["reference_scope"] != component.sequence_scope
        || search["engine"] != component.engine
        || search["engine_version"] != component.engine_version
        || search["parameters"] != crate::genomovar::SEARCH_PARAMETERS
        || search["reference_count"] != component.references.len()
    {
        return Err(fail(
            "report search is incomplete or does not match the verified package",
        ));
    }

    let parent = crate::genomovar::GenomovarParent::from_report(&report);
    if !crate::genomovar::GenomovarParent::provenance_verified(&report) {
        return Err(fail(
            "report parent decision has invalid or incomplete provenance",
        ));
    }

    let rows: Vec<crate::genomovar::MemberRow> =
        serde_json::from_value(search["member_evidence"].clone())
            .map_err(|error| format!("report member_evidence is invalid: {error}"))?;
    let not_returned: Vec<String> =
        serde_json::from_value(search["not_returned_reference_ids"].clone())
            .map_err(|error| format!("report not_returned_reference_ids is invalid: {error}"))?;
    let expected_members: std::collections::BTreeSet<&str> = definition
        .groups()
        .values()
        .flatten()
        .map(String::as_str)
        .collect();
    let mut observed_members = std::collections::BTreeSet::new();
    for member in rows
        .iter()
        .map(|row| row.reference_id.as_str())
        .chain(not_returned.iter().map(String::as_str))
    {
        if !expected_members.contains(member) || !observed_members.insert(member) {
            return Err(fail("report has unexpected or duplicate member accounting"));
        }
    }
    if observed_members != expected_members
        || search["returned_count"] != rows.len()
        || rows.len() + not_returned.len() != expected_members.len()
    {
        return Err(fail(
            "report does not account for the complete genomovar component",
        ));
    }

    let text = |key: &str| search[key].as_str().unwrap_or_default().to_owned();
    let mut context = crate::genomovar::Context {
        parent,
        input_kind: report["input"]["kind"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        query_scope: report["input"]["sequence_scope"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        reference_scope: text("reference_scope"),
        engine: text("engine"),
        engine_version: text("engine_version"),
        parameters: text("parameters"),
        search_status: text("status"),
        adapter_verified: search["adapter_verified"] == true,
        query_sha256: text("query_sha256"),
        package_sha256: text("reference_package_sha256"),
        diagnostic_exclusion: false,
    };
    let baseline = crate::genomovar::assess(&definition, &rows, &context)
        .map_err(|error| error.to_string())?;
    let baseline_value = baseline.to_value();
    for field in [
        "status",
        "genomovar_id",
        "reasons",
        "compatible_groups",
        "incomplete_groups",
        "parent",
    ] {
        if baseline_value[field] != section[field] {
            return Err(fail(
                "saved genomovar assessment does not match its member evidence",
            ));
        }
    }
    if deployment_genomovar_id(&report) != &section["genomovar_id"] {
        return Err(fail(
            "report deployment assignment disagrees with its genomovar result",
        ));
    }

    let parent_recomputation = recompute_diagnostic_parent(args, &report, &package, &mut pins)?;
    if let Some(recomputation) = &parent_recomputation {
        context.parent = crate::genomovar::GenomovarParent::from_report(&recomputation["decision"]);
    }

    let assessment = crate::genomovar::diagnostic_assess(
        &definition,
        &rows,
        &context,
        &args.masked_members,
        &args.removed_groups,
        &args.excluded_evidence_rows,
    )
    .map_err(|error| error.to_string())?;
    if assessment.assigned_genomovar_id().is_some() {
        return Err(fail(
            "diagnostic unexpectedly produced a deployment assignment",
        ));
    }
    let candidate = assessment.would_assign_genomovar_id();
    let output = json!({
        "schema_version": "1.0-terobacter-genomovar-diagnostic",
        "source_report_sha256": report_sha256,
        "reference_package_sha256": expected_package_sha256,
        "genomovar_id": null,
        "diagnostic_would_assign_genomovar_id": candidate,
        "diagnostic_outcome": assessment.to_value()["diagnostic_outcome"],
        "setting": args.setting,
        "parent_recomputation": parent_recomputation,
        "baseline": {
            "genomovar_id": section["genomovar_id"],
            "status": section["status"],
            "compatible_groups": section["compatible_groups"],
            "incomplete_groups": section["incomplete_groups"],
        },
        "diagnostic": assessment.to_value(),
    });

    for (path, hash) in &pins {
        let current_bytes =
            std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
        if sha256_hex(&current_bytes) != *hash {
            return Err(format!("{}: input changed during diagnostic", path.display()).into());
        }
    }
    verify_package(
        &Directory(package_path.clone()),
        expected_package_sha256,
        &RegistryPins::default(),
    )?;
    serde_json::to_string_pretty(&output).map_err(|error| error.to_string().into())
}

/// Read declared files and bind a recomputed parent to the pinned report's original evidence.
fn recompute_diagnostic_parent(
    args: &GenomovarDiagnosticArgs,
    report: &Value,
    package: &VerifiedPackage,
    pins: &mut Pins,
) -> Result<Option<Value>, Failure> {
    let masked_setting = args.setting != "unmasked";
    if masked_setting != args.exclude_references.is_some()
        || (!args.masked_members.is_empty() && !masked_setting)
    {
        return Err("member masks require self_alias or clone_block and a pinned matching primary-reference mask; unmasked forbids a primary mask".into());
    }
    if !masked_setting && args.problem_warnings.is_none() && args.anim_records.is_empty() {
        return Ok(None);
    }
    let mask: Vec<String> = args
        .exclude_references
        .as_ref()
        .map(|path| {
            read(path, args.exclude_references_sha256.as_deref(), pins).map(|text| {
                text.lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
        })
        .transpose()?
        .unwrap_or_default();
    // Members absent from the primary panel have an empty matching mask. The explicit
    // file and its pin remain required; recompute the parent even when no IDs are removed.
    let saved = SavedEvidence::from_json(&report["evidence"].to_string())
        .map_err(|error| format!("report primary evidence is invalid: {error}"))?;
    if saved.provenance().query_sha256 != report["input"]["query_sha256"]
        || saved.provenance().reference_package_sha256 != args.expected_sha256
        || saved.source_sha256() != report["input"]["evidence_sha256"]
    {
        return Err(
            "report primary evidence does not match its query, package or evidence binding".into(),
        );
    }
    let warnings = args
        .problem_warnings
        .as_ref()
        .map(|path| read(path, args.problem_warnings_sha256.as_deref(), pins))
        .transpose()?;
    let saved = saved
        .with_annotations(None, warnings.as_deref(), None)
        .map_err(|error| error.to_string())?;
    verify_artifacts(&saved, pins)?;
    let mut records = AnimRecords::default();
    for path in record_files(&args.anim_records)? {
        let text = read(&path, None, pins)?;
        let parsed = if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            AnimRecords::from_json(&text)
        } else {
            AnimRecords::from_jsonl(&text)
        }
        .map_err(|error| format!("{}: {error}", path.display()))?;
        records.extend(parsed);
    }
    let policy = crate::genomovar::DiagnosticParentPolicy::from_report(report)
        .map_err(|error| error.to_string())?;
    let decision = crate::genomovar::diagnostic_parent(
        saved,
        package,
        &mask,
        &policy,
        &records,
        args.anim_method_profile.as_deref(),
    )
    .map_err(|error| error.to_string())?;
    Ok(Some(json!({"setting": args.setting,
        "primary_mask_sha256": args.exclude_references_sha256,
        "excluded_reference_ids": decision["input"]["mask"]["excluded_reference_ids"],
        "problem_warnings_sha256": args.problem_warnings_sha256,
        "decision": decision})))
}

/// v1 reports keep the genomovar deployment field in `assignment`; v2 reports put it at root.
fn deployment_genomovar_id(report: &Value) -> &Value {
    let target = if report["assignment"].is_object() {
        &report["assignment"]
    } else {
        report
    };
    &target["genomovar_id"]
}

#[cfg(test)]
mod genomovar_diagnostic_report_tests {
    use super::deployment_genomovar_id;
    use serde_json::json;

    #[test]
    fn deployment_genomovar_field_matches_v1_and_v2_report_shapes() {
        let v1 = json!({"assignment": {"genomovar_id": "GV01"}, "genomovar": {}});
        let v2 = json!({"genomovar_id": "GV02", "genomovar": {}});

        assert_eq!(
            deployment_genomovar_id(&v1),
            &v1["assignment"]["genomovar_id"]
        );
        assert_eq!(deployment_genomovar_id(&v2), &v2["genomovar_id"]);
    }
}
