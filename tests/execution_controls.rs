//! Synthetic worker-transport failures independent of optional-limit defaults.
#![cfg(feature = "dev")]

use serde_json::json;
use std::{
    ffi::CString,
    io::{Read, Write},
    os::unix::{
        ffi::OsStrExt,
        process::{CommandExt, ExitStatusExt},
    },
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

/// A failed assertion must not leave a worker behind.
struct Worker(Child);
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
impl Worker {
    fn start(directory: &Path, request: &[u8]) -> Self {
        Self::start_inherited(directory, request, None)
    }

    fn start_inherited(
        directory: &Path,
        request: &[u8],
        restrictions: Option<terobacter::anim::limits::Restrictions>,
    ) -> Self {
        let mut command = Command::new(assert_cmd::cargo::cargo_bin!("pterobacter-dev"));
        if let Some(restrictions) = restrictions {
            unsafe {
                command.pre_exec(move || {
                    for (name, resource) in [
                        ("cpu", libc::RLIMIT_CPU),
                        ("address_space", libc::RLIMIT_AS),
                    ] {
                        #[cfg(target_os = "macos")]
                        if name == "address_space" {
                            continue;
                        }
                        let limit = &restrictions[name];
                        let value = libc::rlimit {
                            rlim_cur: limit.soft.unwrap_or(libc::RLIM_INFINITY),
                            rlim_max: limit.hard.unwrap_or(libc::RLIM_INFINITY),
                        };
                        if libc::setrlimit(resource, &value) != 0 {
                            return Err(std::io::Error::last_os_error());
                        }
                    }
                    Ok(())
                });
            }
        }
        let mut child = command
            .env("TERO_INTERNAL_ANIM_WORKER", "1")
            .current_dir(directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(request).unwrap();
        Self(child)
    }

    fn finish(&mut self) -> ExitStatus {
        let start = Instant::now();
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                return status;
            }
            assert!(start.elapsed() < Duration::from_secs(10), "worker hung");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn stderr(&mut self) -> String {
        let mut text = String::new();
        self.0
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        text
    }
}

#[test]
fn truncated_worker_transport_exits_without_native_output() {
    let directory = tempfile::tempdir().unwrap();
    let mut worker = Worker::start(directory.path(), br#"{"reference":"missing.fa","#);
    assert_eq!(worker.finish().code(), Some(1));
    assert!(worker.stderr().contains("EOF"));
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn worker_rejects_a_parent_other_than_its_live_coordinator() {
    let directory = tempfile::tempdir().unwrap();
    let request = json!({
        "reference": "missing.fa", "query": "missing.fa", "prefix": "native",
        "memory_mib": 6144, "parent_pid": u32::MAX, "wall_seconds": 10.0,
    });
    let mut worker = Worker::start(directory.path(), &serde_json::to_vec(&request).unwrap());
    assert_eq!(worker.finish().code(), Some(1));
    assert!(worker.stderr().contains("parent disappeared"));
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn explicit_worker_wall_watchdog_stops_blocked_native_input() {
    let directory = tempfile::tempdir().unwrap();
    // A FIFO holds native input open without relying on alignment size or machine speed.
    let input = directory.path().join("blocked.fa");
    let name = CString::new(input.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let request = json!({
        "reference": "blocked.fa", "query": "blocked.fa", "prefix": "native",
        "memory_mib": 6144, "parent_pid": std::process::id(), "wall_seconds": 0.05,
    });
    let mut worker = Worker::start(directory.path(), &serde_json::to_vec(&request).unwrap());
    assert_eq!(worker.finish().signal(), Some(libc::SIGKILL));
    assert!(!directory.path().join("native.delta").exists());
    assert!(!directory.path().join("native.filter").exists());
}

fn fifo(root: &Path) {
    let name = CString::new(root.join("blocked.fa").as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
}

fn new_request(
    time: Option<Duration>,
    memory: Option<u64>,
    inherited: terobacter::anim::limits::Restrictions,
) -> serde_json::Value {
    use terobacter::anim::limits::{Execution, WorkerExecution};
    json!({"reference":"blocked.fa", "query":"blocked.fa", "prefix":"native",
        "memory_mib": memory, "parent_pid":std::process::id(), "wall_seconds":time.map(|t|t.as_secs_f64()),
        "execution":WorkerExecution { pair:Execution::new(time,None,memory,inherited).unwrap(), orientation_timeout:time }})
}

fn wait_file(path: &Path, worker: &mut Worker) {
    let start = Instant::now();
    while !path.exists()
        || (path.file_name().is_some_and(|n| n == "limits.json")
            && std::fs::read(path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .is_none())
    {
        if let Some(status) = worker.0.try_wait().unwrap() {
            panic!("worker exited {status}: {}", worker.stderr());
        }
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn unlimited_worker_preserves_os_limits_and_still_accepts_termination() {
    use terobacter::anim::limits::{inherited, WorkerLimits};
    let original = inherited().unwrap();
    let directory = tempfile::tempdir().unwrap();
    fifo(directory.path());
    let request = new_request(None, None, original.clone());
    let mut worker = Worker::start(directory.path(), &serde_json::to_vec(&request).unwrap());
    wait_file(&directory.path().join("limits.json"), &mut worker);
    let limits: WorkerLimits =
        serde_json::from_slice(&std::fs::read(directory.path().join("limits.json")).unwrap())
            .unwrap();
    assert_eq!(limits.effective, original);
    assert_eq!(limits.tool_cpu_seconds, None);
    std::thread::sleep(Duration::from_millis(120));
    assert!(worker.0.try_wait().unwrap().is_none());
    unsafe {
        libc::kill(worker.0.id() as i32, libc::SIGTERM);
    }
    assert_eq!(worker.finish().signal(), Some(libc::SIGTERM));
    assert!(!directory.path().join("stop.json").exists());
    assert_eq!(
        inherited().unwrap(),
        original,
        "worker limits escaped into coordinator"
    );
}

#[test]
fn opt_in_limits_never_raise_inherited_soft_or_hard_limits() {
    use terobacter::anim::limits::{inherited, Restriction, WorkerLimits};
    for hard in [1, 20] {
        let directory = tempfile::tempdir().unwrap();
        fifo(directory.path());
        let mut restrictions = inherited().unwrap();
        restrictions.insert(
            "cpu".into(),
            Restriction {
                soft: Some(hard.min(2)),
                hard: Some(hard),
            },
        );
        #[cfg(target_os = "linux")]
        restrictions.insert(
            "address_space".into(),
            Restriction {
                soft: Some(256 << 20),
                hard: Some(8 << 30),
            },
        );
        let request = new_request(
            Some(Duration::from_secs(10)),
            Some(2048),
            restrictions.clone(),
        );
        let mut worker = Worker::start_inherited(
            directory.path(),
            &serde_json::to_vec(&request).unwrap(),
            Some(restrictions.clone()),
        );
        wait_file(&directory.path().join("limits.json"), &mut worker);
        let limits: WorkerLimits =
            serde_json::from_slice(&std::fs::read(directory.path().join("limits.json")).unwrap())
                .unwrap();
        assert_eq!(
            limits.effective["cpu"],
            Restriction {
                soft: Some(hard.min(2)),
                hard: Some(hard.min(10))
            }
        );
        #[cfg(target_os = "linux")]
        assert_eq!(
            limits.effective["address_space"],
            Restriction {
                soft: Some(256 << 20),
                hard: Some(2048 << 20)
            }
        );
        #[cfg(target_os = "macos")]
        assert_eq!(
            limits.effective["address_space"],
            restrictions["address_space"]
        );
    }
}

#[test]
fn malformed_new_profiles_are_not_unlimited_legacy_requests() {
    for mutation in ["missing", "unknown", "inconsistent"] {
        let directory = tempfile::tempdir().unwrap();
        let mut request = new_request(None, None, terobacter::anim::limits::inherited().unwrap());
        match mutation {
            "missing" => {
                request["execution"]["pair"]
                    .as_object_mut()
                    .unwrap()
                    .remove("memory_mib");
            }
            "unknown" => request["execution"]["pair"]["profile"] = "unknown".into(),
            _ => request["execution"]["orientation_timeout"] = json!({"secs":1,"nanos":0}),
        }
        let mut worker = Worker::start(directory.path(), &serde_json::to_vec(&request).unwrap());
        assert_eq!(worker.finish().code(), Some(1), "{mutation}");
        assert!(!directory.path().join("limits.json").exists());
    }
}

#[test]
fn new_watchdog_keeps_a_specific_marker_instead_of_guessing_from_sigkill() {
    let directory = tempfile::tempdir().unwrap();
    fifo(directory.path());
    let request = new_request(
        Some(Duration::from_millis(50)),
        None,
        terobacter::anim::limits::inherited().unwrap(),
    );
    let mut worker = Worker::start(directory.path(), &serde_json::to_vec(&request).unwrap());
    assert_eq!(worker.finish().signal(), Some(libc::SIGKILL));
    let reason: String =
        serde_json::from_slice(&std::fs::read(directory.path().join("stop.json")).unwrap())
            .unwrap();
    assert_eq!(reason, "worker wall deadline exceeded");
}

#[test]
fn repeated_interrupt_or_termination_stops_a_coordinator_blocked_in_io() {
    for signal in [libc::SIGINT, libc::SIGTERM] {
        let directory = tempfile::tempdir().unwrap();
        fifo(directory.path());
        let output = directory.path().join("evidence");
        let mut coordinator = Worker(
            Command::new(assert_cmd::cargo::cargo_bin!("pterobacter-dev"))
                .args(["anim-pair", "--query"])
                .arg(directory.path().join("blocked.fa"))
                .arg("--reference")
                .arg(directory.path().join("blocked.fa"))
                .arg("--output")
                .arg(&output)
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        wait_file(&output, &mut coordinator);
        unsafe {
            libc::kill(coordinator.0.id() as i32, signal);
        }
        std::thread::sleep(Duration::from_millis(30));
        assert!(coordinator.0.try_wait().unwrap().is_none());
        unsafe {
            libc::kill(coordinator.0.id() as i32, signal);
        }
        assert_eq!(coordinator.finish().signal(), Some(signal));
        assert!(!output.join("result.json").exists());
    }
}

#[test]
fn orphan_worker_exits_when_parent_is_killed() {
    const RELAY: &str = "PTERO_TEST_WORKER_PARENT";
    if let Some(root) = std::env::var_os(RELAY) {
        let root = Path::new(&root);
        let request = new_request(None, None, terobacter::anim::limits::inherited().unwrap());
        let mut worker = Worker::start(root, &serde_json::to_vec(&request).unwrap());
        wait_file(&root.join("limits.json"), &mut worker);
        std::fs::write(root.join("worker.pid"), worker.0.id().to_string()).unwrap();
        loop {
            std::thread::park();
        }
    }
    let directory = tempfile::tempdir().unwrap();
    fifo(directory.path());
    let mut parent = Worker(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "orphan_worker_exits_when_parent_is_killed",
                "--nocapture",
            ])
            .env(RELAY, directory.path())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    wait_file(&directory.path().join("worker.pid"), &mut parent);
    let pid: i32 = std::fs::read_to_string(directory.path().join("worker.pid"))
        .unwrap()
        .parse()
        .unwrap();
    parent.0.kill().unwrap();
    parent.finish();
    let start = Instant::now();
    loop {
        let status = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "stat="])
            .output()
            .unwrap();
        let state = String::from_utf8_lossy(&status.stdout);
        if state.trim().is_empty() || state.trim().starts_with('Z') {
            break;
        }
        if start.elapsed() >= Duration::from_secs(10) {
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
            panic!("orphan worker remains running: {state}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
