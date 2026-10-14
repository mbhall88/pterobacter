//! Batch isolation, report files, output collisions, offline runs, runtime package failure and
//! interruption (AC12), using the synthetic native package only.
use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
mod common;
use common::assembly::native_package;

fn write_query(dir: &Path, name: &str, seq: &[u8]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, [b">query\n".as_slice(), seq, b"\n"].concat()).unwrap();
    path
}

fn v1(dir: &Path, pin: &str) -> assert_cmd::Command {
    let mut command = cargo_bin_cmd!("ptero");
    command
        .arg("--reference-package")
        .arg(dir.join("package"))
        .args(["--expected-sha256", pin]);
    command
}

#[test]
fn concurrent_batch_reports_match_sequential_reports_in_argument_order() {
    let (dir, pin, seq) = native_package();
    let query = write_query(dir.path(), "query.fa", &seq);
    let copy = write_query(dir.path(), "copy.fa", &seq);
    let short = dir.path().join("short.fa");
    let bad = dir.path().join("bad.fa");
    std::fs::write(&short, b">short\nACGT\n").unwrap();
    std::fs::write(&bad, b">bad\n!\n").unwrap();
    let inputs = [&query, &bad, &short, &copy];
    let mut observed = Vec::new();
    for jobs in ["1", "3"] {
        let output = v1(dir.path(), &pin)
            .args(["--jobs", jobs])
            .args(inputs)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(3), "jobs {jobs}");
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        let reports = report.as_array().unwrap();
        assert_eq!(reports.len(), 4);
        for (i, (report, path)) in reports.iter().zip(inputs).enumerate() {
            assert_eq!(report["input"]["ordinal"], i + 1, "{report}");
            assert_eq!(report["input"]["file"], path.to_str().unwrap());
        }
        assert_eq!(reports[0]["assignment"]["species_id"], "SYN-SP01");
        assert_eq!(reports[1]["execution"]["status"], "incomplete");
        assert_eq!(reports[1]["execution"]["stage"], "input");
        assert_eq!(reports[2]["execution"]["status"], "complete");
        assert_eq!(reports[3]["assignment"]["species_id"], "SYN-SP01");
        observed.push(report);
    }
    assert_eq!(observed[0], observed[1]);
}

#[test]
fn zero_jobs_is_a_usage_error() {
    let (dir, pin, seq) = native_package();
    let query = write_query(dir.path(), "query.fa", &seq);
    let output = v1(dir.path(), &pin)
        .args(["--jobs", "0"])
        .arg(&query)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

#[test]
fn report_file_is_written_once_and_never_replaced() {
    let (dir, pin, seq) = native_package();
    let query = write_query(dir.path(), "query.fa", &seq);
    let target = dir.path().join("report.json");
    let output = v1(dir.path(), &pin)
        .arg("--output")
        .arg(&target)
        .arg(&query)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    let first = std::fs::read(&target).unwrap();
    let report: Value = serde_json::from_slice(&first).unwrap();
    assert_eq!(report["assignment"]["species_id"], "SYN-SP01");
    // A second run, even a failing one, must not replace the earlier result.
    let bad = dir.path().join("bad.fa");
    std::fs::write(&bad, b"not FASTA").unwrap();
    let output = v1(dir.path(), &pin)
        .arg("--output")
        .arg(&target)
        .arg(&bad)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
    assert_eq!(std::fs::read(&target).unwrap(), first);
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.contains("partial"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn report_file_must_be_new_in_an_existing_directory() {
    let (dir, pin, seq) = native_package();
    let query = write_query(dir.path(), "query.fa", &seq);
    let original = std::fs::read(&query).unwrap();
    for (target, message) in [
        (query.clone(), "already exists"),
        (dir.path().join("missing/report.json"), "does not exist"),
    ] {
        let output = v1(dir.path(), &pin)
            .arg("--output")
            .arg(&target)
            .arg(&query)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(message));
    }
    assert_eq!(std::fs::read(&query).unwrap(), original);
    assert!(!dir.path().join("missing").exists());
}

#[test]
fn failed_batch_still_writes_its_report_file() {
    let (dir, pin, seq) = native_package();
    let query = write_query(dir.path(), "query.fa", &seq);
    let bad = dir.path().join("bad.fa");
    std::fs::write(&bad, b"not FASTA").unwrap();
    let target = dir.path().join("report.json");
    let output = v1(dir.path(), &pin)
        .arg("--output")
        .arg(&target)
        .args([&query, &bad])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    let report: Value = serde_json::from_slice(&std::fs::read(&target).unwrap()).unwrap();
    assert_eq!(report[0]["assignment"]["species_id"], "SYN-SP01");
    assert_eq!(report[1]["execution"]["status"], "incomplete");
}

#[test]
fn output_collisions_are_refused_before_any_work() {
    let (dir, pin, seq) = native_package();
    let root = dir.path();
    let query = write_query(root, "query.fa", &seq);
    let v2 = ["--policy", "conservative-assembly-anim-v2"];
    let mut cases: Vec<(&str, Vec<std::ffi::OsString>)> = vec![
        (
            "report inside the ANIm evidence directory",
            vec![
                "--anim-output".into(),
                root.join("evidence").into(),
                "--output".into(),
                root.join("evidence/report.json").into(),
            ],
        ),
        (
            "report inside the reference package",
            vec!["--output".into(), root.join("package/report.json").into()],
        ),
        (
            "cache inside the ANIm evidence directory",
            vec![
                "--anim-output".into(),
                root.join("evidence").into(),
                "--anim-cache".into(),
                root.join("evidence/cache").into(),
            ],
        ),
        (
            "ANIm evidence directory inside the cache",
            vec![
                "--anim-cache".into(),
                root.join("cache").into(),
                "--anim-output".into(),
                root.join("cache/evidence").into(),
            ],
        ),
        (
            "ANIm evidence directory inside the reference package",
            vec!["--anim-output".into(), root.join("package/evidence").into()],
        ),
        (
            "same path for the report and the cache",
            vec![
                "--anim-cache".into(),
                root.join("same").into(),
                "--output".into(),
                root.join("same").into(),
            ],
        ),
        (
            "nested through a parent component",
            vec![
                "--anim-output".into(),
                root.join("evidence").into(),
                "--output".into(),
                root.join("other/../evidence/report.json").into(),
            ],
        ),
    ];
    std::os::unix::fs::symlink(root.join("package"), root.join("package-link")).unwrap();
    cases.push((
        "report inside the reference package through a symbolic link",
        vec![
            "--output".into(),
            root.join("package-link/report.json").into(),
        ],
    ));
    let before = inventory(root);
    for (label, args) in cases {
        let output = v1(root, &pin)
            .args(v2)
            .args(&args)
            .arg(&query)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{label}");
        assert!(output.stdout.is_empty(), "{label}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("output collision"),
            "{label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(inventory(root), before, "{label} created or changed files");
    }
}

fn inventory(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_owned()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                found.push((path.clone(), Vec::new()));
                stack.push(path);
            } else {
                found.push((path.clone(), std::fs::read(&path).unwrap()));
            }
        }
    }
    found.sort();
    found
}

#[test]
fn missing_database_preserves_input_evidence_and_exits_4() {
    let (dir, pin, seq) = native_package();
    let query = write_query(dir.path(), "query.fa", &seq);
    std::fs::remove_file(dir.path().join("package/databases/primary_whole/index.db")).unwrap();
    let output = v1(dir.path(), &pin).arg(&query).output().unwrap();
    assert_eq!(output.status.code(), Some(4));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["execution"]["status"], "incomplete");
    assert_eq!(report["execution"]["stage"], "package");
    assert_eq!(report["assignment"]["species_id"], Value::Null);
    assert_eq!(report["input"]["query_sha256"].as_str().unwrap().len(), 64);
}

#[test]
fn missing_package_directory_fails_every_input_with_4() {
    let (dir, pin, seq) = native_package();
    let query = write_query(dir.path(), "query.fa", &seq);
    let copy = write_query(dir.path(), "copy.fa", &seq);
    std::fs::rename(dir.path().join("package"), dir.path().join("moved")).unwrap();
    let output = v1(dir.path(), &pin)
        .args(["--jobs", "2"])
        .args([&query, &copy])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(4));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    for report in report.as_array().unwrap() {
        assert_eq!(report["execution"]["stage"], "package");
        assert_eq!(report["assignment"]["species_id"], Value::Null);
    }
}

/// A named pipe as an assembly input holds the run at a known point: the command blocks reading
/// it, so whatever the test does before writing to the pipe happens mid-run, after every
/// earlier input has completed.
struct Held {
    fifo: PathBuf,
}

impl Held {
    fn new(dir: &Path) -> Self {
        let fifo = dir.join("held.fa");
        let path = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        Self { fifo }
    }

    /// Wait until the command opens the pipe, run `during`, then supply `bytes`.
    fn release(&self, child: &mut std::process::Child, bytes: &[u8], during: impl FnOnce()) {
        let fifo = self.fifo.clone();
        let (opened, receiver) = std::sync::mpsc::channel();
        let (go, proceed) = std::sync::mpsc::channel::<()>();
        let bytes = bytes.to_vec();
        let writer = std::thread::spawn(move || {
            use std::io::Write;
            // Opening for writing blocks until the command opens the pipe for reading.
            let mut pipe = std::fs::OpenOptions::new().write(true).open(fifo).unwrap();
            opened.send(()).unwrap();
            proceed.recv().unwrap();
            pipe.write_all(&bytes).unwrap();
        });
        let start = Instant::now();
        loop {
            if receiver.try_recv().is_ok() {
                break;
            }
            assert!(
                child.try_wait().unwrap().is_none(),
                "command exited before reading the held input"
            );
            assert!(
                start.elapsed() < Duration::from_secs(60),
                "pipe never opened"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        during();
        go.send(()).unwrap();
        writer.join().unwrap();
    }
}

fn spawn(command: assert_cmd::Command) -> std::process::Child {
    use std::process::Stdio;
    std::process::Command::new(command.get_program())
        .args(command.get_args())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

#[test]
fn package_changed_mid_run_invalidates_completed_calls_but_keeps_evidence() {
    let (dir, pin, seq) = native_package();
    let query = write_query(dir.path(), "query.fa", &seq);
    let held = Held::new(dir.path());
    let mut command = v1(dir.path(), &pin);
    command.args([&query, &held.fifo]);
    let mut child = spawn(command);
    let fasta = [b">held\n".as_slice(), &seq, b"\n"].concat();
    held.release(&mut child, &fasta, || {
        std::fs::write(
            dir.path()
                .join("package/databases/primary_whole/markers.bin"),
            b"corrupt",
        )
        .unwrap();
    });
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(4));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    for report in report.as_array().unwrap() {
        assert_eq!(report["execution"]["status"], "incomplete", "{report}");
        assert_eq!(report["execution"]["stage"], "package_recheck");
        assert_eq!(report["assignment"]["species_id"], Value::Null);
        assert!(!report["measurement"]["rows"].as_array().unwrap().is_empty());
    }
}

#[test]
fn interrupted_v1_batch_withholds_every_call_and_keeps_evidence() {
    let (dir, pin, seq) = native_package();
    let query = write_query(dir.path(), "query.fa", &seq);
    let later = write_query(dir.path(), "later.fa", &seq);
    let held = Held::new(dir.path());
    let target = dir.path().join("report.json");
    let mut command = v1(dir.path(), &pin);
    command
        .arg("--output")
        .arg(&target)
        .args([&query, &held.fifo, &later]);
    let mut child = spawn(command);
    let pid = child.id() as i32;
    let fasta = [b">held\n".as_slice(), &seq, b"\n"].concat();
    held.release(&mut child, &fasta, || unsafe {
        libc::kill(pid, libc::SIGINT);
    });
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let report: Value = serde_json::from_slice(&std::fs::read(&target).unwrap()).unwrap();
    let reports = report.as_array().unwrap();
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert_eq!(report["execution"]["status"], "incomplete", "{report}");
        assert_eq!(report["assignment"]["species_id"], Value::Null);
    }
    assert_eq!(reports[0]["execution"]["stage"], "publication");
    assert!(!reports[0]["measurement"]["rows"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(reports[2]["execution"]["error"], "run interrupted");
    assert_eq!(reports[2]["input"]["ordinal"], 3);
}

#[test]
fn interrupted_concurrent_v2_batch_withholds_every_call_and_keeps_pair_evidence() {
    let (dir, pin, seq) = common::assembly::rescue_package();
    let first = write_query(dir.path(), "first.fa", &seq);
    let second = dir.path().join("second.fa");
    std::fs::write(
        &second,
        [b">synthetic\n".as_slice(), &seq, b"\n>short\nACGTN\n"].concat(),
    )
    .unwrap();
    let anim_pin =
        common::package::sha256_hex(&std::fs::read(dir.path().join("fastas/anim.json")).unwrap());
    let target = dir.path().join("report.json");
    let mut command = v1(dir.path(), &pin);
    command
        .args(["--policy", "conservative-assembly-anim-v2", "--jobs", "2"])
        .args(["--expected-anim-sha256", &anim_pin])
        .arg("--anim-reference-package")
        .arg(dir.path().join("fastas"))
        .arg("--anim-output")
        .arg(dir.path().join("evidence"))
        .arg("--output")
        .arg(&target)
        .args([&first, &second]);
    let mut child = spawn(command);
    let started = dir
        .path()
        .join("evidence/query-000001/pair-000001/QR/request.json");
    let start = Instant::now();
    while !started.exists() && child.try_wait().unwrap().is_none() {
        assert!(start.elapsed() < Duration::from_secs(60), "no pair started");
        std::thread::sleep(Duration::from_millis(5));
    }
    unsafe {
        libc::kill(child.id() as i32, libc::SIGINT);
    }
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let report: Value = serde_json::from_slice(&std::fs::read(&target).unwrap()).unwrap();
    for (i, report) in report.as_array().unwrap().iter().enumerate() {
        assert_eq!(report["input"]["ordinal"], i + 1);
        assert_eq!(report["execution"]["status"], "incomplete", "{report}");
        assert_eq!(report["species_id"], Value::Null);
        assert_eq!(report["rescue_status"], "incomplete");
        assert!(!report["measurement"]["rows"].as_array().unwrap().is_empty());
    }
    assert!(started.exists());
    let leftovers = std::fs::read_dir(dir.path())
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("partial")
        })
        .count();
    assert_eq!(leftovers, 0);
}

#[test]
fn the_source_opens_no_socket() {
    let mut stack = vec![
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("native"),
    ];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for needle in ["std::net", "TcpStream", "UdpSocket", "socket(", "<netdb.h>"] {
                assert!(!text.contains(needle), "{} uses {needle}", path.display());
            }
        }
    }
}

#[test]
fn the_locked_dependency_graph_has_no_network_client() {
    let lock = include_str!("../Cargo.lock");
    for name in [
        "reqwest",
        "hyper",
        "ureq",
        "curl",
        "curl-sys",
        "isahc",
        "attohttpc",
        "native-tls",
        "openssl",
        "rustls",
        "h2",
    ] {
        assert!(
            !lock.contains(&format!("name = \"{name}\"\n")),
            "{name} would give the command a network client"
        );
    }
}

/// Run `command` without a network: a new user and network namespace has only an unconfigured
/// loopback interface. Returns `None` where unprivileged namespaces are unavailable.
fn offline(command: &assert_cmd::Command) -> Option<std::process::Output> {
    let probe = std::process::Command::new("unshare")
        .args(["--user", "--map-root-user", "--net", "true"])
        .output();
    if !probe.is_ok_and(|o| o.status.success()) {
        eprintln!("skipping: unprivileged network namespaces are unavailable");
        return None;
    }
    Some(
        std::process::Command::new("unshare")
            .args(["--user", "--map-root-user", "--net", "--"])
            .arg(command.get_program())
            .args(command.get_args())
            .output()
            .unwrap(),
    )
}

#[test]
fn provisioned_v1_run_with_mlst_completes_without_a_network() {
    let (dir, pin, seq) = native_package();
    let scheme = common::mlst::SyntheticScheme::new();
    scheme.write_to(&dir.path().join("mlst"));
    let query = dir.path().join("query.fa");
    let typing = scheme.assembly(&scheme.st5_pieces());
    std::fs::write(
        &query,
        [
            b">chromosome\n".as_slice(),
            &seq,
            b"\n>typing\n",
            &typing,
            b"\n",
        ]
        .concat(),
    )
    .unwrap();
    let mut command = v1(dir.path(), &pin);
    command
        .arg("--mlst-package")
        .arg(dir.path().join("mlst"))
        .args(["--mlst-expected-sha256", &scheme.manifest_sha256()])
        .arg(&query);
    let Some(output) = offline(&command) else {
        return;
    };
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["assignment"]["species_id"], "SYN-SP01");
    assert_eq!(report["mlst"]["sequence_type"], "5");
}

#[test]
fn provisioned_v2_rescue_completes_without_a_network() {
    let (dir, pin, seq) = common::assembly::rescue_package();
    let query = dir.path().join("query.fa");
    std::fs::write(&query, [b">synthetic\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    let anim_pin =
        common::package::sha256_hex(&std::fs::read(dir.path().join("fastas/anim.json")).unwrap());
    let mut command = v1(dir.path(), &pin);
    command
        .args(["--policy", "conservative-assembly-anim-v2"])
        .args(["--expected-anim-sha256", &anim_pin])
        .arg("--anim-reference-package")
        .arg(dir.path().join("fastas"))
        .arg("--anim-output")
        .arg(dir.path().join("evidence"))
        .arg(&query);
    let Some(output) = offline(&command) else {
        return;
    };
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["rescue_status"], "rescued");
    assert_eq!(report["counts"]["completed"], 5);
}
