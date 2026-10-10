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
    },
    /// Reassess saved genomovar member evidence after development-only masks or group removals
    GenomovarDiagnostic {
        /// Completed assembly report produced by terobacter
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
#[derive(Debug, Args)]
struct Inputs {
    /// Saved evidence JSON, schema 2.0.0-dev-evidence
    evidence: PathBuf,
    /// Optional expected SHA-256 of the exact saved evidence bytes
    #[arg(long)]
    evidence_sha256: Option<String>,
    /// Primary inventory from a checked reference package; required for method diagnostics,
    /// for v2 and for the rescue plan
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
/// A reference package to verify and bind the evidence to (v1 only).
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
    #[arg(long, default_value_t = 1800)]
    timeout_seconds: u64,
    #[arg(long, default_value_t = 6144)]
    memory_mib: u64,
}

const EXIT_FAILURE: u8 = 1;
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
        | DevCommand::GenomovarDiagnostic { .. }) => match execute(command) {
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
                timeout: std::time::Duration::from_secs(timeout_seconds),
                memory_mib,
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
extern "C" fn cancel_signal(_: libc::c_int) {
    CANCELLED.store(true, std::sync::atomic::Ordering::SeqCst);
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

/// v2 inputs: evidence, panel, mask and any saved ANIm records, bound by the dry-plan rule.
fn rescue_inputs(
    inputs: &Inputs,
    v2: &V2Inputs,
    pins: &mut Pins,
) -> Result<(RescueInput, BoundAnim, Value), String> {
    let thresholds = v2.thresholds.for_policy(V2_POLICY_ID)?;
    let saved = load(inputs, None, pins)?;
    let Some(inventory) = &inputs.reference_inventory else {
        return Err(format!(
            "{V2_POLICY_ID} needs --reference-inventory, the whole primary panel"
        ));
    };
    let inventory = read(
        inventory,
        inputs.reference_inventory_sha256.as_deref(),
        pins,
    )?;
    let panel = Panel::from_json(&inventory, &saved).map_err(|e| e.to_string())?;
    let mask: Vec<String> = match &v2.exclude_references {
        Some(path) => read(path, None, pins)?
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect(),
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
    Ok((rescue, bound, Value::Array(files)))
}

const UNBOUND: &str =
    "input_and_artifact_hashes_checked; package_authentication_requires_checked_inventory";
const BOUND: &str = "input_and_artifact_hashes_checked; reference_package_verified";

fn execute(command: DevCommand) -> Result<String, Failure> {
    let mut pins = Vec::new();
    let mut verification = UNBOUND;
    let mut result = match command {
        DevCommand::AnimPair(_) | DevCommand::AnimParse { .. } => {
            return Err("ANIm command must be dispatched through run".into());
        }
        DevCommand::GenomovarDiagnostic {
            report,
            expected_report_sha256,
            reference_package,
            expected_sha256,
            masked_members,
            removed_groups,
        } => {
            return genomovar_diagnostic(
                &report,
                &expected_report_sha256,
                &reference_package,
                &expected_sha256,
                &masked_members,
                &removed_groups,
            );
        }
        DevCommand::SavedEvidence {
            inputs,
            policy,
            diagnostics,
            diagnostics_sha256,
            v2,
            package,
        } if policy == V1_POLICY_ID => {
            v2.thresholds.for_policy(V1_POLICY_ID)?;
            if !v2.anim_records.is_empty() || v2.exclude_references.is_some() {
                return Err(format!(
                    "--anim-records and --exclude-references need --policy {V2_POLICY_ID}; \
                     v1 takes method diagnostics through --diagnostics"
                )
                .into());
            }
            if inputs.reference_inventory.is_some() && diagnostics.is_none() {
                return Err("under v1, --reference-inventory pairs with --diagnostics".into());
            }
            let diagnostics = diagnostics
                .as_deref()
                .map(|path| (path, diagnostics_sha256.as_deref()));
            let saved = load(&inputs, diagnostics, &mut pins)?;
            match package.reference_package.zip(package.expected_sha256) {
                Some((directory, expected)) => {
                    let registry_pins = RegistryPins {
                        named_registry_sha256: package.named_registry_sha256,
                        lineage_registry_sha256: package.lineage_registry_sha256,
                    };
                    let package = verify_package(&Directory(directory), &expected, &registry_pins)?;
                    verification = BOUND;
                    decide_bound(&saved, &package, &Policy::v1())?
                        .to_value()
                        .clone()
                }
                None => decide(&saved, &Policy::v1()).to_value().clone(),
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
            if package.reference_package.is_some() {
                return Err(format!(
                    "--reference-package binds v1 evidence only; {V2_POLICY_ID} does not take it yet"
                )
                .into());
            }
            let (rescue, bound, files) = rescue_inputs(&inputs, &v2, &mut pins)?;
            let mut value = rescue
                .decide(&bound)
                .map_err(|e| e.to_string())?
                .to_value()
                .clone();
            value["input"]["anim_record_files"] = files;
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
            );
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
                .map_err(Failure::from);
        }
        DevCommand::RescuePlan { inputs, v2 } => {
            let (rescue, bound, files) = rescue_inputs(&inputs, &v2, &mut pins)?;
            let mut value = rescue
                .plan(&bound)
                .map_err(|e| e.to_string())?
                .to_value()
                .clone();
            value["input"]["anim_record_files"] = files;
            value
        }
    };
    result["input"]["verification"] = Value::String(verification.into());
    let output = serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?;
    // Recheck the originally pinned bytes before a successful result can leave this wrapper.
    for (path, hash) in &pins {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if sha256_hex(&bytes) != *hash {
            return Err(format!("{}: input changed during decision", path.display()).into());
        }
    }
    Ok(output)
}

fn genomovar_diagnostic(
    report_path: &Path,
    expected_report_sha256: &str,
    package_path: &Path,
    expected_package_sha256: &str,
    masked_members: &[String],
    removed_groups: &[String],
) -> Result<String, Failure> {
    let report_bytes = std::fs::read(report_path)
        .map_err(|error| format!("{}: {error}", report_path.display()))?;
    let report_sha256 = sha256_hex(&report_bytes);
    if report_sha256 != expected_report_sha256 {
        return Err(format!(
            "report SHA-256 is {report_sha256}, not the expected {expected_report_sha256}"
        )
        .into());
    }
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
    if !parent.verified
        || parent.status != "assigned"
        || parent.species_id.as_deref() != Some(crate::genomovar::PARENT_SPECIES_ID)
        || parent.subspecies_id.as_deref() != Some(crate::genomovar::PARENT_SUBSPECIES_ID)
        || !crate::genomovar::ACCEPTED_PARENT_POLICIES.contains(&parent.policy_id.as_str())
        || !parent.blocking_reasons.is_empty()
        || !["not_assessed", "explained_after_checks"].contains(&parent.scope_outcome.as_str())
    {
        return Err(fail("report has no verified eligible parent call"));
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
    let context = crate::genomovar::Context {
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

    let assessment = crate::genomovar::diagnostic_assess(
        &definition,
        &rows,
        &context,
        masked_members,
        removed_groups,
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
        "baseline": {
            "genomovar_id": section["genomovar_id"],
            "status": section["status"],
            "compatible_groups": section["compatible_groups"],
            "incomplete_groups": section["incomplete_groups"],
        },
        "diagnostic": assessment.to_value(),
    });

    let current_bytes = std::fs::read(report_path)
        .map_err(|error| format!("{}: {error}", report_path.display()))?;
    if sha256_hex(&current_bytes) != report_sha256 {
        return Err(format!(
            "{}: report changed during diagnostic",
            report_path.display()
        )
        .into());
    }
    serde_json::to_string_pretty(&output).map_err(|error| error.to_string().into())
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
