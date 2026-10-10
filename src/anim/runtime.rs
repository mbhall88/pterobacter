use super::{parse_delta, AnimError, Metrics};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    ffi::{CStr, CString},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::{
        ffi::OsStrExt,
        process::{CommandExt, ExitStatusExt},
    },
    path::{Path, PathBuf},
    process::{Child, Command, ExitCode},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub const METHOD_PROFILE: &str = "reciprocal-mummer4-4.0.1-preserve-n-mum-1-span-v1";
pub const AMENDMENT: &str = "P1-ANIM-MUMMER4-r1";
const WORKER_ENV: &str = "TERO_INTERNAL_ANIM_WORKER";
const MAX_INPUT_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Method {
    pub profile: String,
    pub amendment: String,
    pub upstream_commit: String,
    pub build_sha256: String,
    pub parser_sha256: String,
    pub source_sha256: String,
    pub compiler: String,
    pub target: String,
    pub implementation_version: String,
    pub implementation_sha256: String,
    pub query_threads: u32,
    pub reference_n_policy: String,
    pub filter_rng: String,
    pub memory_policy: String,
}
impl Method {
    fn current(executable: &Path) -> Result<Self, AnimError> {
        Ok(Self {
            profile: METHOD_PROFILE.into(),
            amendment: AMENDMENT.into(),
            upstream_commit: "eb734606f2d516f42a0e0dce7a116bfb88ec1ebf".into(),
            build_sha256: env!("TERO_ANIM_BUILD_SHA256").into(),
            parser_sha256: hash_bytes(include_bytes!("metrics.rs")),
            source_sha256: env!("TERO_ANIM_SOURCE_SHA256").into(),
            compiler: env!("TERO_ANIM_COMPILER").into(),
            target: env!("TERO_ANIM_TARGET").into(),
            implementation_version: env!("CARGO_PKG_VERSION").into(),
            implementation_sha256: hash_file(executable)?,
            query_threads: 1,
            reference_n_policy: "preserved; literal character identity; known long-N limitation"
                .into(),
            filter_rng: "isolated worker process; libc srand(1); one-to-one flag1to1(-1,100)"
                .into(),
            memory_policy:
                "Linux: RLIMIT_AS; macOS: Mach peak RSS watchdog at 10 ms; failures retained".into(),
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputBinding {
    pub id: String,
    /// Exact supplied file bytes, including compression when present.
    pub sha256: String,
    /// Decoded, formatting-normalized FASTA bytes read by the native worker.
    pub native_sha256: String,
    pub total_bases: u64,
    pub scope: String,
    pub component: String,
    pub concept_ids: Vec<String>,
    pub record_lengths: BTreeMap<String, u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub argv: Vec<String>,
    pub implementation_sha256: String,
    pub environment: BTreeMap<String, String>,
    pub worker_pid: u32,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub stop_reason: Option<String>,
    pub native_exit_code: Option<i32>,
    pub native_error: Option<String>,
    pub wall_seconds: f64,
    pub cpu_seconds: Option<f64>,
    pub peak_rss_bytes: Option<u64>,
    pub artifacts: BTreeMap<String, String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Orientation {
    pub method_profile: String,
    pub native_reference_id: String,
    pub native_query_id: String,
    pub status: String,
    pub metrics: Option<Metrics>,
    pub error: Option<String>,
    pub receipt: Option<Receipt>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairRecord {
    pub schema: String,
    pub status: String,
    pub query: Option<InputBinding>,
    pub reference: Option<InputBinding>,
    pub method: Method,
    pub orientations: BTreeMap<String, Orientation>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairRequest {
    pub query: PathBuf,
    pub reference: PathBuf,
    pub output: PathBuf,
    pub query_id: Option<String>,
    pub reference_id: Option<String>,
    pub reference_component: String,
    pub reference_concept_ids: Vec<String>,
    pub expect_query_sha256: Option<String>,
    pub expect_reference_sha256: Option<String>,
    pub timeout: Duration,
    pub memory_mib: u64,
}

/// Input identities from the caller's current query and reference context.
/// These must not be taken from the saved pair being verified.
#[derive(Debug, Clone)]
pub struct ExpectedPair {
    pub query_id: String,
    pub query_sha256: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub reference_component: String,
    pub reference_concept_ids: Vec<String>,
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn hash_file(path: &Path) -> Result<String, AnimError> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn invalid(message: impl Into<String>) -> AnimError {
    AnimError::Invalid(message.into())
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), AnimError> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<(), AnimError> {
    let mut text = serde_json::to_vec_pretty(value)?;
    text.push(b'\n');
    write_new(path, &text)
}
fn lengths(bytes: &[u8]) -> Result<BTreeMap<String, u64>, AnimError> {
    let text = std::str::from_utf8(bytes).map_err(|_| invalid("FASTA is not UTF-8"))?;
    let mut records = BTreeMap::<String, u64>::new();
    let mut current: Option<String> = None;
    for line in text.lines().map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(header) = line.strip_prefix('>') {
            if current.as_ref().is_some_and(|id| records[id] == 0) {
                return Err(invalid("empty FASTA record"));
            }
            let id = header
                .split_whitespace()
                .next()
                .ok_or_else(|| invalid("empty FASTA ID"))?;
            if id.contains('\0') || records.insert(id.into(), 0).is_some() {
                return Err(invalid("duplicate or invalid FASTA ID"));
            }
            current = Some(id.into());
        } else {
            let id = current
                .as_ref()
                .ok_or_else(|| invalid("sequence before FASTA header"))?;
            if !line
                .bytes()
                .all(|b| b"ACGTRYSWKMBDHVN".contains(&b.to_ascii_uppercase()))
            {
                return Err(invalid("invalid nucleotide alphabet"));
            }
            let n = u64::try_from(line.len()).map_err(|_| AnimError::Overflow)?;
            let length = records.get_mut(id).unwrap();
            *length = length.checked_add(n).ok_or(AnimError::Overflow)?;
        }
    }
    if records.is_empty() || records.values().any(|n| *n == 0) {
        return Err(invalid("empty FASTA file or record"));
    }
    Ok(records)
}
// Only formatting is removed. Every validated sequence byte, including Ns,
// stays in its original record and order. Both native parsers see LF lines.
fn native_fasta(bytes: &[u8]) -> Result<Vec<u8>, AnimError> {
    let text = std::str::from_utf8(bytes).map_err(|_| invalid("FASTA is not UTF-8"))?;
    let mut output = Vec::new();
    for line in text.lines().map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(header) = line.strip_prefix('>') {
            let id = header
                .split_whitespace()
                .next()
                .ok_or_else(|| invalid("empty FASTA ID"))?;
            output.push(b'>');
            output.extend_from_slice(id.as_bytes());
        } else {
            output.extend_from_slice(line.as_bytes());
        }
        output.push(b'\n');
    }
    Ok(output)
}

// Preserve plain-input formatting and historical native hashes. For compressed
// inputs, use the same content-based decoder as the public assembly reader and
// retain record bytes before the existing validation/normalization steps.
fn decoded_fasta(bytes: &[u8]) -> Result<Cow<'_, [u8]>, AnimError> {
    if bytes.iter().copied().find(|b| !b.is_ascii_whitespace()) == Some(b'>') {
        return Ok(Cow::Borrowed(bytes));
    }
    let mut reader = needletail::parse_fastx_reader(std::io::Cursor::new(bytes))
        .map_err(|e| invalid(e.to_string()))?;
    let mut decoded = Vec::new();
    while let Some(record) = reader.next() {
        let record = record.map_err(|e| invalid(e.to_string()))?;
        if record.format() != needletail::parser::Format::Fasta {
            return Err(invalid("input must be assembly FASTA, not FASTQ"));
        }
        let size = decoded
            .len()
            .checked_add(record.all().len())
            .and_then(|n| n.checked_add(1))
            .ok_or(AnimError::Overflow)?;
        if size as u64 > MAX_INPUT_BYTES {
            return Err(invalid("decoded FASTA exceeds the 256-MiB per-input limit"));
        }
        decoded.extend_from_slice(record.all());
        decoded.push(b'\n');
    }
    Ok(Cow::Owned(decoded))
}
fn snapshot(
    path: &Path,
    dest: &Path,
    id: Option<&str>,
    expected: Option<&str>,
    role: &str,
    component: &str,
    concept_ids: Vec<String>,
) -> Result<InputBinding, AnimError> {
    if fs::metadata(path)?.len() > MAX_INPUT_BYTES {
        return Err(invalid("FASTA exceeds the 256-MiB per-input limit"));
    }
    // Bound the read even if the original changes after metadata inspection.
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err(invalid("FASTA exceeds the 256-MiB per-input limit"));
    }
    let digest = hash_bytes(&bytes);
    write_new(dest, &bytes)?;
    if expected.is_some_and(|value| value != digest) {
        return Err(invalid(format!("{role} SHA256 mismatch")));
    }
    let decoded = decoded_fasta(&bytes)?;
    let record_lengths = lengths(&decoded)?;
    let native = native_fasta(&decoded)?;
    write_new(&dest.with_extension("native.fa"), &native)?;
    let total_bases = record_lengths
        .values()
        .try_fold(0_u64, |a, b| a.checked_add(*b).ok_or(AnimError::Overflow))?;
    let id = id.unwrap_or(&digest);
    if id.is_empty() || id.contains('\0') || component.is_empty() {
        return Err(invalid("empty or invalid input identity/component"));
    }
    Ok(InputBinding {
        id: id.into(),
        sha256: digest,
        native_sha256: hash_bytes(&native),
        total_bases,
        scope: "whole_assembly".into(),
        component: component.into(),
        concept_ids,
        record_lengths,
    })
}

/// Execute exactly one reciprocal pair. A new output directory is required;
/// failures retain an incomplete record and never become biological negatives.
pub fn measure_pair(
    request: &PairRequest,
    cancelled: &AtomicBool,
) -> Result<PairRecord, AnimError> {
    if request.timeout.is_zero()
        || request.timeout > Duration::from_secs(1800)
        || !(64..=6144).contains(&request.memory_mib)
    {
        return Err(invalid(
            "timeout must be within 0..1800 seconds; worker memory within 64..6144 MiB",
        ));
    }
    fs::create_dir(&request.output)?;
    let root = fs::canonicalize(&request.output)?;
    let executable = std::env::current_exe()?;
    let mut report = PairRecord {
        schema: "terobacter-anim-pair-v1".into(),
        status: "incomplete".into(),
        query: None,
        reference: None,
        method: Method::current(&executable)?,
        orientations: BTreeMap::new(),
        error: None,
    };
    let start = Instant::now();
    let result = (|| -> Result<(), AnimError> {
        write_json(&root.join("request.json"), request)?;
        fs::create_dir(root.join("inputs"))?;
        report.query = Some(snapshot(
            &request.query,
            &root.join("inputs/query.fa"),
            request.query_id.as_deref(),
            request.expect_query_sha256.as_deref(),
            "query",
            "query-whole-assembly",
            vec![],
        )?);
        report.reference = Some(snapshot(
            &request.reference,
            &root.join("inputs/reference.fa"),
            request.reference_id.as_deref(),
            request.expect_reference_sha256.as_deref(),
            "reference",
            &request.reference_component,
            request.reference_concept_ids.clone(),
        )?);
        let query = report.query.as_ref().unwrap();
        let reference = report.reference.as_ref().unwrap();
        for direction in ["QR", "RQ"] {
            if cancelled.load(Ordering::SeqCst) {
                return Err(invalid("cancelled"));
            }
            if start.elapsed() >= request.timeout {
                return Err(invalid("reciprocal pair deadline exceeded"));
            }
            let (native_ref, native_query) = if direction == "QR" {
                (reference, query)
            } else {
                (query, reference)
            };
            let directory = root.join(direction);
            fs::create_dir(&directory)?;
            let mut orientation = Orientation {
                method_profile: METHOD_PROFILE.into(),
                native_reference_id: native_ref.id.clone(),
                native_query_id: native_query.id.clone(),
                status: "error".into(),
                metrics: None,
                error: None,
                receipt: None,
            };
            let work = (|| -> Result<(), AnimError> {
                let worker = WorkerRequest {
                    reference: if direction == "QR" {
                        "../inputs/reference.native.fa"
                    } else {
                        "../inputs/query.native.fa"
                    }
                    .into(),
                    query: if direction == "QR" {
                        "../inputs/query.native.fa"
                    } else {
                        "../inputs/reference.native.fa"
                    }
                    .into(),
                    prefix: "native".into(),
                    memory_mib: request.memory_mib,
                    parent_pid: std::process::id(),
                    wall_seconds: request
                        .timeout
                        .saturating_sub(start.elapsed())
                        .as_secs_f64(),
                };
                let receipt = execute(
                    &executable,
                    &directory,
                    &worker,
                    start,
                    request.timeout,
                    cancelled,
                    &report.method.implementation_sha256,
                )?;
                let stopped = receipt.stop_reason.clone();
                let failed = receipt.exit_code != Some(0)
                    || receipt.native_exit_code != Some(0)
                    || receipt.native_error.is_some();
                orientation.receipt = Some(receipt);
                if let Some(reason) = stopped {
                    return Err(invalid(reason));
                }
                if failed {
                    return Err(invalid("native worker failed; see retained receipt/stderr"));
                }
                let raw = fs::read_to_string(directory.join("native.filter"))?;
                let mut metrics = parse_delta(
                    &raw,
                    &native_ref.record_lengths,
                    &native_query.record_lengths,
                )?;
                if direction == "RQ" {
                    std::mem::swap(&mut metrics.reference_covered, &mut metrics.query_covered);
                    std::mem::swap(&mut metrics.reference_length, &mut metrics.query_length);
                }
                orientation.status = if metrics.blocks > 0 {
                    "aligned"
                } else {
                    "no_alignment"
                }
                .into();
                orientation.metrics = Some(metrics);
                Ok(())
            })();
            if let Err(error) = work {
                orientation.error = Some(error.to_string());
                report.orientations.insert(direction.into(), orientation);
                return Err(error);
            }
            report.orientations.insert(direction.into(), orientation);
        }
        if hash_file(&root.join("inputs/query.fa"))? != query.sha256
            || hash_file(&root.join("inputs/reference.fa"))? != reference.sha256
            || hash_file(&root.join("inputs/query.native.fa"))? != query.native_sha256
            || hash_file(&root.join("inputs/reference.native.fa"))? != reference.native_sha256
        {
            return Err(invalid("snapshot changed during native execution"));
        }
        Ok(())
    })();
    match result {
        Ok(()) => report.status = "complete".into(),
        Err(error) => report.error = Some(error.to_string()),
    }
    if let ("incomplete", Some(query), Some(reference)) = (
        report.status.as_str(),
        report.query.as_ref(),
        report.reference.as_ref(),
    ) {
        for direction in ["QR", "RQ"] {
            let (native_ref, native_query) = if direction == "QR" {
                (reference, query)
            } else {
                (query, reference)
            };
            report
                .orientations
                .entry(direction.into())
                .or_insert(Orientation {
                    method_profile: METHOD_PROFILE.into(),
                    native_reference_id: native_ref.id.clone(),
                    native_query_id: native_query.id.clone(),
                    status: "not_run".into(),
                    metrics: None,
                    error: report.error.clone(),
                    receipt: None,
                });
        }
    }
    write_json(&root.join("result.json"), &report)?;
    Ok(report)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerRequest {
    reference: PathBuf,
    query: PathBuf,
    prefix: PathBuf,
    memory_mib: u64,
    parent_pid: u32,
    wall_seconds: f64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerResult {
    exit_code: i32,
    message: Option<String>,
    cpu_seconds: f64,
    peak_rss_bytes: u64,
}
struct ChildGuard {
    child: Child,
    finished: bool,
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !self.finished {
            unsafe {
                libc::kill(-(self.child.id() as i32), libc::SIGKILL);
            }
            let _ = self.child.wait();
        }
    }
}
fn execute(
    executable: &Path,
    directory: &Path,
    request: &WorkerRequest,
    start: Instant,
    timeout: Duration,
    cancelled: &AtomicBool,
    implementation_sha256: &str,
) -> Result<Receipt, AnimError> {
    write_json(&directory.join("request.json"), request)?;
    let stdout = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("worker.stdout"))?;
    let stderr = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("worker.stderr"))?;
    let environment = BTreeMap::from([
        (WORKER_ENV.into(), "1".into()),
        ("LC_ALL".into(), "C".into()),
    ]);
    let child = Command::new(executable)
        .current_dir(directory)
        .envs(&environment)
        .stdin(File::open(directory.join("request.json"))?)
        .stdout(stdout)
        .stderr(stderr)
        .process_group(0)
        .spawn()?;
    let mut guard = ChildGuard {
        child,
        finished: false,
    };
    let worker_start = Instant::now();
    let mut stop_reason = None;
    let status = loop {
        if let Some(status) = guard.child.try_wait()? {
            guard.finished = true;
            break status;
        }
        let reason = if cancelled.load(Ordering::SeqCst) {
            Some("cancelled")
        } else if start.elapsed() >= timeout {
            Some("reciprocal pair deadline exceeded")
        } else {
            None
        };
        if let Some(reason) = reason {
            stop_reason = Some(reason.to_owned());
            unsafe {
                libc::kill(-(guard.child.id() as i32), libc::SIGKILL);
            }
            let status = guard.child.wait()?;
            guard.finished = true;
            break status;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let result = fs::read(directory.join("worker.stdout"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<WorkerResult>(&bytes).ok());
    if status.success()
        && result
            .as_ref()
            .is_none_or(|r| !r.cpu_seconds.is_finite() || r.cpu_seconds < 0.0)
    {
        stop_reason = Some("missing or invalid native completion receipt".into());
    }
    let mut artifacts = BTreeMap::new();
    for file in [
        "request.json",
        "worker.stdout",
        "worker.stderr",
        "native.delta",
        "native.filter",
    ] {
        let path = directory.join(file);
        if path.is_file() {
            artifacts.insert(file.into(), hash_file(&path)?);
        }
    }
    let receipt = Receipt {
        argv: vec![executable.to_string_lossy().into_owned()],
        implementation_sha256: implementation_sha256.into(),
        environment,
        worker_pid: guard.child.id(),
        exit_code: status.code(),
        signal: status.signal(),
        stop_reason,
        native_exit_code: result.as_ref().map(|r| r.exit_code),
        native_error: result.as_ref().and_then(|r| r.message.clone()),
        wall_seconds: worker_start.elapsed().as_secs_f64(),
        cpu_seconds: result.as_ref().map(|r| r.cpu_seconds),
        peak_rss_bytes: result.as_ref().map(|r| r.peak_rss_bytes),
        artifacts,
    };
    write_json(&directory.join("receipt.json"), &receipt)?;
    Ok(receipt)
}

unsafe extern "C" {
    fn tero_anim_orientation(
        reference: *const libc::c_char,
        query: *const libc::c_char,
        prefix: *const libc::c_char,
        error: *mut libc::c_char,
        capacity: usize,
    ) -> i32;
}
/// Private worker transport, shared by all executable entry points. This is
/// one native orientation, not a public evaluation operation or extra binary.
pub fn run_worker_if_requested() -> Option<ExitCode> {
    if std::env::var_os(WORKER_ENV).as_deref() != Some(std::ffi::OsStr::new("1")) {
        return None;
    }
    Some(match worker() {
        Ok(result) => {
            println!("{}", serde_json::to_string(&result).unwrap());
            if result.exit_code == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    })
}
fn worker() -> Result<WorkerResult, AnimError> {
    let request: WorkerRequest = serde_json::from_reader(std::io::stdin().lock().take(65536))?;
    if !(64..=6144).contains(&request.memory_mib)
        || !request.wall_seconds.is_finite()
        || request.wall_seconds <= 0.0
        || request.wall_seconds > 1800.0
        || request.parent_pid <= 1
    {
        return Err(invalid("invalid worker resource limits or parent"));
    }
    // Check the parent before and after installing the Linux death signal to
    // close the race where the coordinator exits before prctl is called.
    if unsafe { libc::getppid() } as u32 != request.parent_pid {
        return Err(invalid("worker parent disappeared"));
    }
    #[cfg(target_os = "linux")]
    if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let parent = request.parent_pid;
    let deadline = Duration::from_secs_f64(request.wall_seconds);
    let memory_ceiling = request.memory_mib * 1024 * 1024;
    std::thread::spawn(move || {
        let start = Instant::now();
        loop {
            if unsafe { libc::getppid() } as u32 != parent
                || start.elapsed() >= deadline
                || memory_exceeded(memory_ceiling)
            {
                unsafe {
                    libc::kill(libc::getpid(), libc::SIGKILL);
                }
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    let cpu = libc::rlimit {
        rlim_cur: request.wall_seconds.ceil() as libc::rlim_t,
        rlim_max: request.wall_seconds.ceil() as libc::rlim_t,
    };
    if unsafe { libc::setrlimit(libc::RLIMIT_CPU, &cpu) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    #[cfg(target_os = "linux")]
    {
        let limit = libc::rlimit {
            rlim_cur: memory_ceiling,
            rlim_max: memory_ceiling,
        };
        if unsafe { libc::setrlimit(libc::RLIMIT_AS, &limit) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    // Do not allow native crashes to leave large core files.
    let core = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::setrlimit(libc::RLIMIT_CORE, &core) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let strings = [&request.reference, &request.query, &request.prefix]
        .map(|p| CString::new(p.as_os_str().as_bytes()).map_err(|_| invalid("NUL in native path")));
    let [reference, query, prefix] = strings;
    let (reference, query, prefix) = (reference?, query?, prefix?);
    let mut error = [0_i8; 1024];
    let code = unsafe {
        tero_anim_orientation(
            reference.as_ptr(),
            query.as_ptr(),
            prefix.as_ptr(),
            error.as_mut_ptr(),
            error.len(),
        )
    };
    let message = if code != 0 {
        Some(
            unsafe { CStr::from_ptr(error.as_ptr()) }
                .to_string_lossy()
                .into_owned(),
        )
    } else {
        None
    };
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let usage = unsafe { usage.assume_init() };
    let cpu_seconds = (usage.ru_utime.tv_sec + usage.ru_stime.tv_sec) as f64
        + (usage.ru_utime.tv_usec + usage.ru_stime.tv_usec) as f64 / 1e6;
    let peak = u64::try_from(usage.ru_maxrss).map_err(|_| invalid("negative peak RSS"))?;
    let peak_rss_bytes = if cfg!(target_os = "macos") {
        peak
    } else {
        peak.checked_mul(1024).ok_or(AnimError::Overflow)?
    };
    if peak_rss_bytes > memory_ceiling {
        return Err(invalid("native worker exceeded memory ceiling"));
    }
    Ok(WorkerResult {
        exit_code: code,
        message,
        cpu_seconds,
        peak_rss_bytes,
    })
}

#[cfg(target_os = "linux")]
fn memory_exceeded(_: u64) -> bool {
    false
}

#[cfg(target_os = "macos")]
#[allow(deprecated)] // libc supplies this small Mach ABI; no additional native crate.
fn memory_exceeded(ceiling: u64) -> bool {
    // macOS maps a large virtual address range before alignment. RLIMIT_AS
    // below that existing mapping returns EINVAL. Bound peak resident bytes
    // instead, including transient peaks between samples, and fail closed.
    let mut info = std::mem::MaybeUninit::<libc::mach_task_basic_info>::uninit();
    let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
    let code = unsafe {
        libc::task_info(
            libc::mach_task_self(),
            libc::MACH_TASK_BASIC_INFO,
            info.as_mut_ptr().cast(),
            &mut count,
        )
    };
    if code != libc::KERN_SUCCESS {
        eprintln!("error: cannot sample native worker memory");
        return true;
    }
    let exceeded = unsafe { info.assume_init() }.resident_size_max > ceiling;
    if exceeded {
        eprintln!("error: native worker exceeded memory ceiling");
    }
    exceeded
}

/// Validate a saved complete pair before reuse. The caller supplies expected
/// identities, byte hashes, component and concepts from its current context.
/// The retained request, artifacts, roles, integers and current method/build
/// bindings are checked again. Package authentication remains the caller's job.
pub fn verify_pair(directory: &Path, expected: &ExpectedPair) -> Result<PairRecord, AnimError> {
    let report: PairRecord = serde_json::from_slice(&fs::read(directory.join("result.json"))?)?;
    let mut current = Method::current(&std::env::current_exe()?)?;
    // The two public aliases and development binary share the same library,
    // but have different executable hashes. Keep the producing binary receipt
    // while checking the current library's method and native build identity.
    current.implementation_sha256 = report.method.implementation_sha256.clone();
    if report.schema != "terobacter-anim-pair-v1"
        || report.status != "complete"
        || report.error.is_some()
        || report.method != current
    {
        return Err(invalid(
            "stale, incomplete or incompatible method provenance",
        ));
    }
    let query = report
        .query
        .as_ref()
        .ok_or_else(|| invalid("missing query binding"))?;
    let reference = report
        .reference
        .as_ref()
        .ok_or_else(|| invalid("missing reference binding"))?;
    if query.sha256 != expected.query_sha256 || reference.sha256 != expected.reference_sha256 {
        return Err(invalid("stale input provenance"));
    }
    if query.id != expected.query_id
        || query.component != "query-whole-assembly"
        || !query.concept_ids.is_empty()
        || reference.id != expected.reference_id
        || reference.component != expected.reference_component
        || reference.concept_ids != expected.reference_concept_ids
    {
        return Err(invalid("input identity/component/concept mismatch"));
    }
    let request: PairRequest = serde_json::from_slice(&fs::read(directory.join("request.json"))?)?;
    if request.query_id.as_deref().unwrap_or(&query.sha256) != expected.query_id
        || request.reference_id.as_deref().unwrap_or(&reference.sha256) != expected.reference_id
        || request.reference_component != expected.reference_component
        || request.reference_concept_ids != expected.reference_concept_ids
        || request
            .expect_query_sha256
            .as_ref()
            .is_some_and(|hash| hash != &expected.query_sha256)
        || request
            .expect_reference_sha256
            .as_ref()
            .is_some_and(|hash| hash != &expected.reference_sha256)
    {
        return Err(invalid("saved operation context mismatch"));
    }
    for (input, name) in [(query, "query.fa"), (reference, "reference.fa")] {
        let bytes = fs::read(directory.join("inputs").join(name))?;
        let decoded = decoded_fasta(&bytes)?;
        let native = native_fasta(&decoded)?;
        let native_path = directory
            .join("inputs")
            .join(name)
            .with_extension("native.fa");
        if hash_bytes(&bytes) != input.sha256
            || hash_bytes(&native) != input.native_sha256
            || fs::read(native_path)? != native
            || lengths(&decoded)? != input.record_lengths
            || input.scope != "whole_assembly"
            || input
                .record_lengths
                .values()
                .try_fold(0_u64, |a, b| a.checked_add(*b).ok_or(AnimError::Overflow))?
                != input.total_bases
        {
            return Err(invalid("snapshot binding mismatch"));
        }
    }
    if report.orientations.len() != 2 {
        return Err(invalid("incomplete reciprocal orientations"));
    }
    for direction in ["QR", "RQ"] {
        let orientation = report
            .orientations
            .get(direction)
            .ok_or_else(|| invalid("missing orientation"))?;
        let (r, q) = if direction == "QR" {
            (reference, query)
        } else {
            (query, reference)
        };
        if orientation.method_profile != METHOD_PROFILE
            || orientation.native_reference_id != r.id
            || orientation.native_query_id != q.id
        {
            return Err(invalid("reciprocal role binding mismatch"));
        }
        let receipt = orientation
            .receipt
            .as_ref()
            .ok_or_else(|| invalid("missing native receipt"))?;
        if receipt.exit_code != Some(0)
            || receipt.signal.is_some()
            || receipt.stop_reason.is_some()
            || receipt.native_exit_code != Some(0)
            || receipt.native_error.is_some()
            || receipt.implementation_sha256 != report.method.implementation_sha256
        {
            return Err(invalid("failed native receipt"));
        }
        let worker: WorkerRequest =
            serde_json::from_slice(&fs::read(directory.join(direction).join("request.json"))?)?;
        let expected_paths = if direction == "QR" {
            ("../inputs/reference.native.fa", "../inputs/query.native.fa")
        } else {
            ("../inputs/query.native.fa", "../inputs/reference.native.fa")
        };
        if worker.reference != Path::new(expected_paths.0)
            || worker.query != Path::new(expected_paths.1)
            || worker.prefix != Path::new("native")
        {
            return Err(invalid("native input role mismatch"));
        }
        let completion: WorkerResult =
            serde_json::from_slice(&fs::read(directory.join(direction).join("worker.stdout"))?)?;
        if completion.exit_code != 0
            || completion.message.is_some()
            || !completion.cpu_seconds.is_finite()
            || completion.cpu_seconds < 0.0
            || receipt.cpu_seconds != Some(completion.cpu_seconds)
            || receipt.peak_rss_bytes != Some(completion.peak_rss_bytes)
        {
            return Err(invalid("invalid native completion"));
        }
        for file in [
            "request.json",
            "worker.stdout",
            "worker.stderr",
            "native.delta",
            "native.filter",
        ] {
            let expected = receipt
                .artifacts
                .get(file)
                .ok_or_else(|| invalid("missing artifact hash"))?;
            if &hash_file(&directory.join(direction).join(file))? != expected {
                return Err(invalid("stale native artifact"));
            }
        }
        let mut metrics = parse_delta(
            &fs::read_to_string(directory.join(direction).join("native.filter"))?,
            &r.record_lengths,
            &q.record_lengths,
        )?;
        if direction == "RQ" {
            std::mem::swap(&mut metrics.reference_covered, &mut metrics.query_covered);
            std::mem::swap(&mut metrics.reference_length, &mut metrics.query_length);
        }
        if orientation.metrics.as_ref() != Some(&metrics)
            || orientation.status
                != if metrics.blocks > 0 {
                    "aligned"
                } else {
                    "no_alignment"
                }
            || orientation.error.is_some()
        {
            return Err(invalid("saved integer record mismatch"));
        }
    }
    Ok(report)
}
