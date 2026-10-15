//! Synthetic worker-transport failures independent of optional-limit defaults.
#![cfg(feature = "dev")]

use serde_json::json;
use std::{
    ffi::CString,
    io::{Read, Write},
    os::unix::{ffi::OsStrExt, process::ExitStatusExt},
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
        let mut child = Command::new(assert_cmd::cargo::cargo_bin!("pterobacter-dev"))
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
