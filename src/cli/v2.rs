//! Bounded native execution of the pure rescue plan. No taxonomic rules live here.
use super::cache::{copy_tree, AnimCache, CacheKey, Lookup, ReferenceKey, Stored};
use crate::{
    anim::{
        self,
        limits::{self, Execution, Restrictions},
        ExpectedPair, PairRequest,
    },
    reference::{
        anim::{verify_anim_reference, AnimReference},
        VerifiedPackage,
    },
    rescue::{AnimRecords, BoundAnim, RescueInput},
};
use clap::Args;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

#[derive(Debug, Args)]
pub struct V2Settings {
    /// Reference genome FASTAs for MUMmer4 rescue; needed when alignments are triggered
    #[arg(short = 'a', long)]
    pub anim_reference_package: Option<PathBuf>,
    /// Trusted SHA-256 of the component's anim.json
    #[arg(short = 'A', long, value_parser = super::parse_sha256)]
    pub expected_anim_sha256: Option<String>,
    /// New directory for alignments and execution records; see the assembly guide
    #[arg(short = 'e', long)]
    pub anim_output: Option<PathBuf>,
    /// Directory of verified reciprocal pairs to reuse and extend; created when absent
    #[arg(short = 'c', long)]
    pub anim_cache: Option<PathBuf>,
    /// Opt-in reciprocal pair wall limit in seconds; omitted or 0 means unlimited
    #[arg(short = 'T', long, value_parser = parse_pair_seconds)]
    pub anim_pair_seconds: Option<Duration>,
    /// Opt-in ANIm-stage budget per assembly in seconds; omitted or 0 means unlimited
    #[arg(short = 'Q', long, value_parser = parse_query_seconds)]
    pub anim_query_seconds: Option<Duration>,
    /// Opt-in native worker ceiling in MiB; omitted or 0 means unlimited
    #[arg(short = 'L', long, value_parser = parse_memory)]
    pub anim_memory_mib: Option<u64>,
}
pub(crate) fn parse_seconds(text: &str) -> Result<Duration, String> {
    let seconds: f64 = text
        .parse()
        .map_err(|_| "limit must be finite nonnegative seconds")?;
    if !seconds.is_finite() || seconds < 0.0 || (seconds > 0.0 && seconds < 1e-9) {
        return Err("limit must be zero or finite seconds of at least one nanosecond".into());
    }
    let mantissa = text.split(['e', 'E']).next().unwrap_or(text);
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let significant = digits.trim_start_matches('0');
    if !significant.is_empty() {
        let exponent = text
            .split(['e', 'E'])
            .nth(1)
            .unwrap_or("0")
            .parse::<i64>()
            .map_err(|_| "time exponent is not representable")?;
        let fractional = mantissa.split('.').nth(1).map_or(0, str::len);
        let order = i128::from(exponent) - fractional as i128 + significant.len() as i128 - 1;
        if seconds == 0.0 || text.starts_with('-') || order < -9 {
            return Err("nonzero time must be positive and at least one nanosecond".into());
        }
    }
    let duration = Duration::try_from_secs_f64(seconds).map_err(|e| e.to_string())?;
    if !duration.is_zero() {
        limits::validate_duration(duration)?;
    }
    Ok(duration)
}
fn parse_pair_seconds(text: &str) -> Result<Duration, String> {
    parse_seconds(text)
}
fn parse_query_seconds(text: &str) -> Result<Duration, String> {
    parse_seconds(text)
}
pub(crate) fn parse_memory(text: &str) -> Result<u64, String> {
    let mib = text
        .parse::<u64>()
        .map_err(|_| "memory must be nonnegative integer MiB")?;
    if mib != 0 {
        limits::memory_bytes(mib)?;
    }
    Ok(mib)
}
impl V2Settings {
    fn pair_timeout(&self) -> Option<Duration> {
        self.anim_pair_seconds.filter(|time| !time.is_zero())
    }
    fn query_timeout(&self) -> Option<Duration> {
        self.anim_query_seconds.filter(|time| !time.is_zero())
    }
    fn memory(&self) -> Option<u64> {
        self.anim_memory_mib.filter(|mib| *mib != 0)
    }
    pub fn supplied(&self) -> bool {
        self.anim_reference_package.is_some()
            || self.expected_anim_sha256.is_some()
            || self.anim_output.is_some()
            || self.anim_cache.is_some()
            || self.anim_pair_seconds.is_some()
            || self.anim_query_seconds.is_some()
            || self.anim_memory_mib.is_some()
    }
}
pub struct V2Context<'a> {
    pub settings: &'a V2Settings,
    pub thresholds: &'a crate::rescue::Thresholds,
    pub query_ordinal: usize,
    pub cancelled: &'a AtomicBool,
}
pub struct NativeRun {
    pub bound: BoundAnim,
    pub plan: Value,
    pub execution: Value,
    pub error: Option<String>,
}

pub fn execute(
    input: &RescueInput,
    package: &VerifiedPackage,
    bytes: &[u8],
    context: &V2Context<'_>,
) -> NativeRun {
    let mut run = NativeRun {
        bound: BoundAnim::none(),
        plan: Value::Null,
        execution: json!({"method_profile": anim::METHOD_PROFILE, "amendment": anim::AMENDMENT,
            "diagnostic_origin": "generated", "execution_profile": limits::EXECUTION_PROFILE,
            "configured_pair_timeout": context.settings.pair_timeout(), "configured_query_budget": context.settings.query_timeout(), "pair_workers": 1, "query_threads": 1,
            "pair_seconds": context.settings.pair_timeout().map(|time| time.as_secs_f64()),
            "query_seconds": context.settings.query_timeout().map(|time| time.as_secs_f64()),
            "memory_mib": context.settings.memory(),
            "pairs": [], "stop_reason": "not_required"}),
        error: None,
    };
    let started = Instant::now();
    let inherited = limits::inherited();
    let mut result = match inherited {
        Ok(inherited) => {
            run.execution["inherited_restrictions"] = json!(inherited);
            execute_inner(input, package, bytes, context, &mut run, inherited)
        }
        Err(error) => Err(error.to_string()),
    };
    run.execution["wall_seconds"] = json!(started.elapsed().as_secs_f64());
    let omitted = run.plan["requested_pairs"]
        .as_array()
        .map(|pairs| {
            pairs
                .iter()
                .filter(|p| p["status"] == "outstanding")
                .map(|p| p["reference_id"].clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    run.execution["not_run_reference_ids"] = json!(omitted);
    if result.is_ok() {
        // Include final reporting work, but not later skani/genomovar/MLST stages.
        let charged = run.execution["charged_cache_duration"].clone();
        let charged: Duration = serde_json::from_value(charged).unwrap_or_default();
        result = query_checkpoint(context, started, charged, &mut run);
    }
    if let Err(error) = result {
        run.error = Some(error);
    }
    run
}
fn execute_inner(
    input: &RescueInput,
    package: &VerifiedPackage,
    bytes: &[u8],
    context: &V2Context<'_>,
    run: &mut NativeRun,
    inherited: Restrictions,
) -> Result<(), String> {
    let start = Instant::now();
    run.plan = input
        .plan(&run.bound)
        .map_err(|e| e.to_string())?
        .to_value()
        .clone();
    if context.cancelled.load(Ordering::SeqCst) {
        run.execution["stop_reason"] = "cancelled".into();
        return Err("ANIm run interrupted".into());
    }
    query_checkpoint(context, start, Duration::ZERO, run)?;
    if run.plan["requested_pairs"]
        .as_array()
        .expect("plan pairs")
        .is_empty()
    {
        return Ok(());
    }
    run.execution["stop_reason"] = "missing_component".into();
    let settings = context.settings;
    let directory = settings
        .anim_reference_package
        .as_ref()
        .ok_or("required ANIm FASTA component is missing")?;
    let expected = settings
        .expected_anim_sha256
        .as_deref()
        .ok_or("required trusted ANIm manifest digest is missing")?;
    run.execution["stop_reason"] = "component_integrity".into();
    let component = verify_component(directory, expected, package)?;
    query_checkpoint(context, start, Duration::ZERO, run)?;
    run.execution["reference_component"] = json!({"directory": directory, "manifest_sha256": component.manifest_sha256,
        "reference_package_sha256": component.reference_package_sha256, "licence_sha256": component.licence_sha256});
    run.execution["stop_reason"] = "cache_unavailable".into();
    let cache = settings
        .anim_cache
        .as_deref()
        .map(AnimCache::open)
        .transpose()?;
    run.execution["cache"] = json!(settings.anim_cache);
    run.execution["stop_reason"] = "missing_output".into();
    let output = settings
        .anim_output
        .as_ref()
        .ok_or("required fresh ANIm evidence directory is missing")?
        .join(format!("query-{:06}", context.query_ordinal));
    fs::create_dir(&output).map_err(|e| e.to_string())?;
    let output = fs::canonicalize(output).map_err(|e| e.to_string())?;
    let query = output.join("query.fa");
    fs::write(&query, bytes).map_err(|e| e.to_string())?;
    query_checkpoint(context, start, Duration::ZERO, run)?;
    let query_sha = anim::hash_bytes(bytes);
    let mut records = AnimRecords::default();
    let budget = settings.query_timeout();
    // Charge recorded native time as a defined admission rule. This is not a prediction
    // of how long fresh execution would take on this machine.
    let mut reused_time = Duration::ZERO;
    loop {
        run.plan = input
            .plan(&run.bound)
            .map_err(|e| e.to_string())?
            .to_value()
            .clone();
        query_checkpoint(context, start, reused_time, run)?;
        if !run.plan["decisive_vetoes"]
            .as_array()
            .expect("veto list")
            .is_empty()
        {
            run.execution["stop_reason"] = "decisive_veto".into();
            break;
        }
        let next = run.plan["requested_pairs"]
            .as_array()
            .expect("plan pairs")
            .iter()
            .find(|p| p["status"] == "outstanding");
        let Some(next) = next else {
            run.execution["stop_reason"] = "plan_complete".into();
            break;
        };
        let mut remaining =
            budget.map(|time| time.saturating_sub(start.elapsed().saturating_add(reused_time)));
        if remaining.is_some_and(|time| time.is_zero()) {
            run.execution["stop_reason"] = "query_deadline".into();
            return Err("ANIm query wall budget exhausted".into());
        }
        let id = next["reference_id"].as_str().expect("plan reference");
        let reference = component
            .reference(id)
            .ok_or("requested ANIm reference is absent")?;
        let pair_output = output.join(format!("pair-{:06}", records.len() + 1));
        let expected_pair = ExpectedPair {
            query_id: query_sha.clone(),
            query_sha256: query_sha.clone(),
            reference_id: id.into(),
            reference_sha256: reference.sha256.clone(),
            reference_component: "primary_whole".into(),
            reference_concept_ids: reference.concept_ids.clone(),
        };
        let mut request = PairRequest {
            query: query.clone(),
            reference: directory.join(&reference.path),
            output: pair_output.clone(),
            query_id: Some(query_sha.clone()),
            reference_id: Some(id.into()),
            reference_component: "primary_whole".into(),
            reference_concept_ids: reference.concept_ids.clone(),
            expect_query_sha256: Some(query_sha.clone()),
            expect_reference_sha256: Some(reference.sha256.clone()),
            timeout: limits::minimum(settings.pair_timeout(), remaining),
            memory_mib: settings.memory(),
            execution: Some(
                Execution::new(
                    settings.pair_timeout(),
                    remaining,
                    settings.memory(),
                    inherited.clone(),
                )
                .map_err(|e| e.to_string())?,
            ),
        };
        run.execution["stop_reason"] = "pair_failure".into();
        let entry = json!({"reference_id": id, "stage": next["stage"], "directory": pair_output, "status": "failed", "record": null});
        run.execution["pairs"]
            .as_array_mut()
            .expect("pairs")
            .push(entry);
        let index = run.execution["pairs"].as_array().expect("pairs").len() - 1;
        let key = cache.as_ref().map(|_| {
            CacheKey::new(
                &query_sha,
                ReferenceKey {
                    reference_id: id.into(),
                    sha256: reference.sha256.clone(),
                    component: "primary_whole".into(),
                    concept_ids: reference.concept_ids.clone(),
                    scope: "whole_assembly_as_deposited".into(),
                    package_sha256: component.reference_package_sha256.clone(),
                },
                request.execution.as_ref().expect("new execution profile"),
            )
        });
        let mut lookup = cache.as_ref().zip(key.as_ref()).map(|(c, k)| c.lookup(k));
        if let Some(key) = &key {
            run.execution["pairs"][index]["cache"] =
                json!({"status": "miss", "key_sha256": key.sha256()});
        }
        // Lookup includes verification and may consume stage time. Derive admission again.
        query_checkpoint(context, start, reused_time, run)?;
        remaining =
            budget.map(|time| time.saturating_sub(start.elapsed().saturating_add(reused_time)));
        if remaining.is_some_and(|time| time.is_zero()) {
            run.execution["stop_reason"] = "query_deadline".into();
            return Err("ANIm query wall budget exhausted during cache verification".into());
        }
        request.execution = Some(
            Execution::new(
                settings.pair_timeout(),
                remaining,
                settings.memory(),
                inherited.clone(),
            )
            .map_err(|e| e.to_string())?,
        );
        request.timeout = request.execution.as_ref().unwrap().effective_pair_timeout;
        // A saved pair stands only within the limit this pair runs under now.
        let saved = match lookup.take() {
            Some(Lookup::Hit(saved)) => match native_time(&saved) {
                Ok(time) if request.timeout.is_none_or(|limit| time <= limit) => {
                    Some((saved, time))
                }
                Ok(_) => {
                    lookup = Some(Lookup::Rejected(
                        "saved native time exceeds the time now remaining for this pair".into(),
                    ));
                    None
                }
                Err(why) => {
                    lookup = Some(Lookup::Rejected(why));
                    None
                }
            },
            other => {
                lookup = other;
                None
            }
        };
        query_checkpoint(context, start, reused_time, run)?;
        // A reused pair is copied into this query's evidence and verified there like a new one.
        let mut copy_checkpoint_error = None;
        let pair = if let Some((saved, time)) = saved {
            run.execution["pairs"][index]["cache"]["status"] = "reused".into();
            run.execution["pairs"][index]["current_admission"] = json!({
                "remaining_query_budget": remaining, "effective_pair_timeout": request.timeout, "charged_duration": time});
            reused_time = if budget.is_some() {
                reused_time
                    .checked_add(time)
                    .ok_or("cached time accounting overflow")?
            } else {
                Duration::ZERO
            };
            run.execution["charged_cache_duration"] = json!(reused_time);
            copy_tree(&saved, &pair_output).map_err(|e| e.to_string())?;
            // Verify the copied evidence even after expiry, then return the checkpoint error.
            copy_checkpoint_error = query_checkpoint(context, start, reused_time, run).err();
            serde_json::from_slice(
                &fs::read(pair_output.join("result.json")).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?
        } else {
            if let Some(Lookup::Rejected(why)) = &lookup {
                run.execution["pairs"][index]["cache"]["status"] = "rejected".into();
                run.execution["pairs"][index]["cache"]["rejection"] = why.clone().into();
            }
            anim::measure_pair(&request, context.cancelled).map_err(|e| e.to_string())?
        };
        run.execution["pairs"][index]["record"] =
            serde_json::to_value(&pair).map_err(|e| e.to_string())?;
        let verified = if pair.status == "complete" {
            match anim::verify_pair(&pair_output, &expected_pair) {
                Ok(verified) => {
                    run.execution["pairs"][index]["status"] = "complete".into();
                    Some(verified)
                }
                Err(e) => {
                    run.execution["pairs"][index]["verification_error"] = e.to_string().into();
                    if run.execution["pairs"][index]["cache"]["status"] == "reused" {
                        run.execution["pairs"][index]["cache"]["status"] = "rejected".into();
                        run.execution["pairs"][index]["cache"]["rejection"] =
                            "copied pair failed verification".into();
                    }
                    None
                }
            }
        } else {
            None
        };
        if let (Some(cache), Some(key)) = (&cache, &key) {
            let status = &mut run.execution["pairs"][index]["cache"];
            // Only a new, verified complete pair is saved; failures never enter the cache.
            if status["status"] == "miss" {
                status["status"] = match verified.as_ref().map(|_| cache.store(key, &pair_output)) {
                    None => "not_stored".into(),
                    Some(Ok(Stored::Stored)) => "stored".into(),
                    Some(Ok(Stored::Existing)) => "already_stored".into(),
                    Some(Err(why)) => {
                        status["store_error"] = why.into();
                        "store_failed".into()
                    }
                };
            }
        }
        if let Some(error) = copy_checkpoint_error {
            return Err(error);
        }
        query_checkpoint(context, start, reused_time, run)?;
        // Failed and partially measured pairs still carry diagnostic evidence. Integrity-rejected
        // complete records are withheld from the decision; actual attempts remain in the receipts.
        if pair.status != "complete" || verified.is_some() {
            records.extend(
                AnimRecords::from_json(&serde_json::to_string(&pair).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?,
            );
            run.bound = input
                .bind_supplied(&records, anim::METHOD_PROFILE)
                .map_err(|e| e.to_string())?;
            run.plan = input
                .plan(&run.bound)
                .map_err(|e| e.to_string())?
                .to_value()
                .clone();
        }
        if verified.is_none() {
            if context.cancelled.load(Ordering::SeqCst) {
                run.execution["stop_reason"] = "cancelled".into();
            }
            return Err(pair
                .error
                .unwrap_or_else(|| "ANIm pair failed integrity checks".into()));
        }
    }
    let stop_reason = run.execution["stop_reason"].clone();
    run.execution["stop_reason"] = "component_recheck".into();
    verify_component(directory, expected, package)?;
    query_checkpoint(context, start, reused_time, run)?;
    run.execution["stop_reason"] = stop_reason;
    run.execution["wall_seconds"] = json!(start.elapsed().as_secs_f64());
    Ok(())
}

/// Native wall time recorded in a saved pair's two worker receipts.
fn native_time(pair: &Path) -> Result<Duration, String> {
    let record: anim::PairRecord =
        serde_json::from_slice(&fs::read(pair.join("result.json")).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let seconds: f64 = record
        .orientations
        .values()
        .filter_map(|o| o.receipt.as_ref())
        .map(|r| r.wall_seconds)
        .sum();
    Duration::try_from_secs_f64(seconds).map_err(|e| format!("saved native time: {e}"))
}

pub(super) fn verify_component(
    directory: &Path,
    expected: &str,
    package: &VerifiedPackage,
) -> Result<AnimReference, String> {
    if fs::symlink_metadata(directory)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("symbolic link component root is refused".into());
    }
    verify_anim_reference(&super::Directory(directory.to_owned()), expected, package)
}

/// The publication boundary is after all final hash rechecks. An interruption before
/// this point invalidates completed calls while preserving their measurement receipts.
pub fn cancel_before_publication(reports: &mut [Value], cancelled: &AtomicBool) -> bool {
    if !cancelled.load(Ordering::SeqCst) {
        return false;
    }
    for report in reports {
        if report["execution"]["status"] == "complete" {
            report["execution"] = json!({"status": "incomplete", "stage": "publication", "error": "run interrupted before publication"});
        } else {
            report["execution"]["cancelled_before_publication"] = true.into();
        }
        super::withhold(report);
    }
    true
}

fn query_checkpoint(
    context: &V2Context<'_>,
    start: Instant,
    reused: Duration,
    run: &mut NativeRun,
) -> Result<(), String> {
    if context.cancelled.load(Ordering::SeqCst) {
        run.execution["stop_reason"] = "cancelled".into();
        return Err("ANIm run interrupted".into());
    }
    if context
        .settings
        .query_timeout()
        .is_some_and(|budget| start.elapsed().saturating_add(reused) >= budget)
    {
        run.execution["stop_reason"] = "query_deadline".into();
        return Err("ANIm query wall budget exhausted after cooperative checkpoint".into());
    }
    Ok(())
}
