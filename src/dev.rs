//! Files, argument parsing and artifact verification for the feature-gated development binary.
use crate::decision::{decide, Policy, V1_POLICY_ID};
use crate::evidence::{sha256_hex, SavedEvidence};
use crate::panel::Panel;
use crate::rescue::{AnimRecords, BoundAnim, RescueInput, Thresholds, V2_POLICY_ID};
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
    },
    /// Dry v2 rescue plan: trigger, nearest parent, witnesses and the ANIm pairs still
    /// required. Reads saved evidence and records only; runs no alignment
    RescuePlan {
        #[command(flatten)]
        inputs: Inputs,
        #[command(flatten)]
        v2: V2Inputs,
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
        | DevCommand::SkaniSearch { .. }
        | DevCommand::RescuePlan { .. }) => match execute(command) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(message) => {
                eprintln!("error: {message}");
                ExitCode::FAILURE
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
    let rescue =
        RescueInput::new(saved, panel, &mask, Thresholds::standard()).map_err(|e| e.to_string())?;
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

fn execute(command: DevCommand) -> Result<String, String> {
    let mut pins = Vec::new();
    let mut result = match command {
        DevCommand::AnimPair(_) | DevCommand::AnimParse { .. } => {
            return Err("ANIm command must be dispatched through run".into());
        }
        DevCommand::SavedEvidence {
            inputs,
            policy,
            diagnostics,
            diagnostics_sha256,
            v2,
        } if policy == V1_POLICY_ID => {
            if !v2.anim_records.is_empty() || v2.exclude_references.is_some() {
                return Err(format!(
                    "--anim-records and --exclude-references need --policy {V2_POLICY_ID}; \
                     v1 takes method diagnostics through --diagnostics"
                ));
            }
            if inputs.reference_inventory.is_some() && diagnostics.is_none() {
                return Err("under v1, --reference-inventory pairs with --diagnostics".into());
            }
            let diagnostics = diagnostics
                .as_deref()
                .map(|path| (path, diagnostics_sha256.as_deref()));
            let saved = load(&inputs, diagnostics, &mut pins)?;
            decide(&saved, &Policy::v1()).to_value().clone()
        }
        DevCommand::SavedEvidence {
            inputs,
            diagnostics,
            v2,
            ..
        } => {
            if diagnostics.is_some() {
                return Err(format!(
                    "{V2_POLICY_ID} reads ANIm through --anim-records, not v1 --diagnostics"
                ));
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
                .and_then(|report| {
                    serde_json::to_string_pretty(&report).map_err(|e| e.to_string())
                });
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
    result["input"]["verification"] = Value::String(
        "input_and_artifact_hashes_checked; package_authentication_requires_checked_inventory"
            .into(),
    );
    let output = serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?;
    // Recheck the originally pinned bytes before a successful result can leave this wrapper.
    for (path, hash) in &pins {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if sha256_hex(&bytes) != *hash {
            return Err(format!("{}: input changed during decision", path.display()));
        }
    }
    Ok(output)
}
