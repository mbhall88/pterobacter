use super::*;
use crate::evidence::sha256_hex;
use flate2::{write::GzEncoder, Compression};
use std::io::Cursor;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::sync::{mpsc, Arc, Barrier};
use tempfile::TempDir;

fn private_temp() -> TempDir {
    tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap()
}

// Generated transport-only fixtures: no biological sequences or frozen scientific data.
fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}
fn header(path: &str, kind: u8, bytes: u64) -> tar::Header {
    let mut h = tar::Header::new_ustar();
    h.as_mut_bytes()[..path.len()].copy_from_slice(path.as_bytes());
    h.set_size(bytes);
    h.set_mode(0o7777); // Input modes, ownership and timestamps are never installed.
    h.set_entry_type(tar::EntryType::new(kind));
    h.set_cksum();
    h
}
fn raw_tar(entries: &[(&str, u8, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    for (path, kind, bytes) in entries {
        out.extend_from_slice(header(path, *kind, bytes.len() as u64).as_bytes());
        out.extend_from_slice(bytes);
        out.resize(out.len().div_ceil(512) * 512, 0);
    }
    out.resize(out.len() + 1024, 0);
    out
}
fn fixture() -> (InstallPlan, Vec<u8>) {
    let bytes = gzip(&raw_tar(&[(
        "sub/data.bin",
        b'0',
        b"pinned fixture payload",
    )]));
    let plan = InstallPlan {
        version: "fixture-1".into(),
        archives: vec![ArchivePin {
            component: "primary".into(),
            archive_root: None,
            bytes: bytes.len() as u64,
            sha256: sha256_hex(&bytes),
            tar_bytes_limit: 4096,
            files: vec![FilePin {
                path: "sub/data.bin".into(),
                bytes: 22,
                sha256: sha256_hex(b"pinned fixture payload"),
            }],
        }],
    };
    (plan, bytes)
}
fn fetch(bytes: Vec<u8>) -> impl FnMut(&ArchivePin) -> io::Result<Box<dyn Read>> {
    move |_| Ok(Box::new(Cursor::new(bytes.clone())))
}
fn accept(_: &Path) -> io::Result<()> {
    Ok(())
}
fn clean(root: &Path) {
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
}
fn pinned(plan: &mut InstallPlan, bytes: &[u8]) {
    plan.archives[0].bytes = bytes.len() as u64;
    plan.archives[0].sha256 = sha256_hex(bytes);
}

#[test]
fn installs_atomically_verifies_and_reuses_without_fetching() {
    let (plan, bytes) = fixture();
    let root = private_temp();
    let installed = install(root.path(), &plan, fetch(bytes), |path| {
        assert!(!root.path().join(&plan.version).exists());
        assert_eq!(
            fs::read(path.join("primary/sub/data.bin"))?,
            b"pinned fixture payload"
        );
        Ok(())
    })
    .unwrap();
    assert!(matches!(installed, Installed::New(_)));
    let reused = install(
        root.path(),
        &plan,
        |_| panic!("offline reuse must not fetch"),
        accept,
    )
    .unwrap();
    assert_eq!(reused, Installed::Reused(installed.path().to_owned()));
    let metadata = fs::metadata(installed.path().join("primary/sub/data.bin")).unwrap();
    assert_eq!(metadata.mode() & 0o7777, 0o600);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn archive_size_and_digest_are_independent_of_payload() {
    for variant in 0..4 {
        let (plan, mut bytes) = fixture();
        match variant {
            0 => {
                bytes[10] ^= 1;
            }
            1 => {
                bytes.truncate(bytes.len() / 2);
            }
            2 => {
                bytes.push(0);
            }
            3 => {
                bytes.clear();
            }
            _ => unreachable!(),
        }
        let root = private_temp();
        assert!(install(root.path(), &plan, fetch(bytes), accept).is_err());
        clean(root.path());
    }
}

#[test]
fn trusted_archive_still_needs_all_member_hashes_and_contents() {
    let variants = [
        vec![("sub/data.bin", b'0', b"changed".as_slice())],
        vec![("unexpected.bin", b'0', b"pinned fixture payload".as_slice())],
        vec![],
        vec![("sub/data.bin", b'0', b"pinned fixture payloae".as_slice())],
        vec![
            ("sub/data.bin", b'0', b"pinned fixture payload".as_slice()),
            ("extra", b'0', b"x".as_slice()),
        ],
    ];
    for members in variants {
        let (mut plan, _) = fixture();
        let bytes = gzip(&raw_tar(&members));
        pinned(&mut plan, &bytes);
        let root = private_temp();
        assert!(install(root.path(), &plan, fetch(bytes), accept).is_err());
        clean(root.path());
    }
}

#[test]
fn rejects_absolute_traversal_alias_and_windows_paths() {
    for path in [
        "/outside",
        "../outside",
        "sub/../../outside",
        "sub/../data.bin",
        "sub//data.bin",
        "./sub/data.bin",
        "sub\\data.bin",
        "C:/data.bin",
    ] {
        let (mut plan, _) = fixture();
        let bytes = gzip(&raw_tar(&[(path, b'0', b"pinned fixture payload")]));
        pinned(&mut plan, &bytes);
        let root = private_temp();
        assert!(
            install(root.path(), &plan, fetch(bytes), accept).is_err(),
            "accepted {path}"
        );
        clean(root.path());
    }
}

#[test]
fn rejects_links_devices_fifos_sparse_and_extension_headers() {
    for kind in [
        b'1', b'2', b'3', b'4', b'6', b'7', b'S', b'L', b'K', b'x', b'g',
    ] {
        let (mut plan, _) = fixture();
        let bytes = gzip(&raw_tar(&[(
            "sub/data.bin",
            kind,
            b"pinned fixture payload",
        )]));
        pinned(&mut plan, &bytes);
        let root = private_temp();
        assert!(
            install(root.path(), &plan, fetch(bytes), accept).is_err(),
            "accepted type {kind}"
        );
        clean(root.path());
    }
}

#[test]
fn rejects_duplicate_members_but_accepts_expected_directory_entries() {
    let (mut plan, _) = fixture();
    let root = private_temp();
    let entries = [
        ("sub/", b'5', b"".as_slice()),
        ("sub/data.bin", b'0', b"pinned fixture payload".as_slice()),
    ];
    let bytes = gzip(&raw_tar(&entries));
    pinned(&mut plan, &bytes);
    install(root.path(), &plan, fetch(bytes), accept).unwrap();
    for entries in [
        vec![entries[1], entries[1]],
        vec![entries[0], entries[0], entries[1]],
        vec![("wrong/", b'5', b"".as_slice()), entries[1]],
    ] {
        let bytes = gzip(&raw_tar(&entries));
        pinned(&mut plan, &bytes);
        let root = private_temp();
        assert!(install(root.path(), &plan, fetch(bytes), accept).is_err());
        clean(root.path());
    }
}

#[test]
fn checks_decoding_ceiling_crc_and_trailing_data() {
    for variant in 0..6 {
        let (mut plan, _) = fixture();
        let mut raw = raw_tar(&[("sub/data.bin", b'0', b"pinned fixture payload")]);
        match variant {
            0 => raw.resize(32 * 1024, 0),
            1 => raw.push(b'x'),
            2 => raw.truncate(1024), // no tar terminator
            _ => {}
        }
        let mut bytes = gzip(&raw);
        if variant == 3 {
            let index = bytes.len() - 8;
            bytes[index] ^= 1;
        }
        if variant == 4 {
            bytes.truncate(bytes.len() - 8);
        }
        if variant == 5 {
            bytes.extend(gzip(&[0; 1024]));
        }
        pinned(&mut plan, &bytes);
        let root = private_temp();
        assert!(
            install(root.path(), &plan, fetch(bytes), accept).is_err(),
            "variant {variant}"
        );
        clean(root.path());
    }
}

#[test]
fn missing_or_corrupt_existing_install_never_falls_back_or_replaces() {
    for variant in 0..5 {
        let (plan, bytes) = fixture();
        let root = private_temp();
        let target = install(root.path(), &plan, fetch(bytes), accept).unwrap();
        let path = target.path().join("primary/sub/data.bin");
        match variant {
            0 => fs::remove_file(&path).unwrap(),
            1 => fs::write(&path, b"changed").unwrap(),
            2 => fs::write(target.path().join("unlisted"), b"x").unwrap(),
            3 => fs::create_dir(target.path().join("unlisted-dir")).unwrap(),
            4 => {
                fs::remove_file(&path).unwrap();
                symlink("/nonexistent", &path).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(install(
            root.path(),
            &plan,
            |_| panic!("must not download on verification failure"),
            accept
        )
        .is_err());
        assert!(target.path().exists());
    }
}

#[test]
fn incompatible_scientific_components_never_become_visible() {
    let (plan, bytes) = fixture();
    let root = private_temp();
    let error = install(root.path(), &plan, fetch(bytes), |_| {
        Err(invalid("scientific binding mismatch"))
    })
    .unwrap_err();
    assert!(error.to_string().contains("binding mismatch"));
    clean(root.path());
}

#[test]
fn second_component_failure_rolls_back_the_whole_release() {
    let (mut plan, bytes) = fixture();
    let mut second = plan.archives[0].clone();
    second.component = "genomes".into();
    plan.archives.push(second);
    let root = private_temp();
    assert!(install(
        root.path(),
        &plan,
        |pin| {
            if pin.component == "genomes" {
                Err(io::Error::other("interrupted download"))
            } else {
                Ok(Box::new(Cursor::new(bytes.clone())))
            }
        },
        accept
    )
    .is_err());
    clean(root.path());
}

#[test]
fn insufficient_space_fails_before_fetch_and_keeps_previous_versions() {
    let (plan, bytes) = fixture();
    let root = private_temp();
    install(root.path(), &plan, fetch(bytes.clone()), accept).unwrap();
    let mut next = plan.clone();
    next.version = "fixture-2".into();
    let error = install_with_space(
        root.path(),
        &next,
        |_| panic!("no space"),
        accept,
        |_| Ok(0),
    )
    .unwrap_err();
    assert!(error.to_string().contains("insufficient"));
    verify(&root.path().join(&plan.version), &plan, accept).unwrap();
    assert!(!root.path().join(&next.version).exists());
}

#[test]
fn interruptions_clean_only_the_current_attempt() {
    struct Interrupted;
    impl Read for Interrupted {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
        }
    }
    let (plan, _) = fixture();
    let root = private_temp();
    let old = root.path().join(".fixture-1.staging-old");
    fs::create_dir(&old).unwrap();
    fs::write(old.join("retain"), b"previous interrupted attempt").unwrap();
    assert!(install(root.path(), &plan, |_| Ok(Box::new(Interrupted)), accept).is_err());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    assert_eq!(
        fs::read(old.join("retain")).unwrap(),
        b"previous interrupted attempt"
    );
}

#[test]
fn concurrent_same_version_is_exclusive_and_other_versions_can_proceed() {
    let (plan, bytes) = fixture();
    let root = private_temp();
    let barrier = Arc::new(Barrier::new(2));
    let first_barrier = barrier.clone();
    let first_root = root.path().to_owned();
    let first_plan = plan.clone();
    let first_bytes = bytes.clone();
    let (tx, rx) = mpsc::channel();
    let first = std::thread::spawn(move || {
        install(
            &first_root,
            &first_plan,
            move |_| {
                tx.send(()).unwrap();
                first_barrier.wait();
                Ok(Box::new(Cursor::new(first_bytes.clone())) as Box<dyn Read>)
            },
            accept,
        )
    });
    rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
    let error = install(root.path(), &plan, |_| panic!("locked"), accept).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    let mut other = plan.clone();
    other.version = "fixture-2".into();
    install(root.path(), &other, fetch(bytes), accept).unwrap();
    barrier.wait();
    first.join().unwrap().unwrap();
    verify(&root.path().join(&plan.version), &plan, accept).unwrap();
    verify(&root.path().join(&other.version), &other, accept).unwrap();
}

#[test]
fn existing_lock_is_never_guessed_stale() {
    let (plan, _) = fixture();
    let root = private_temp();
    let lock = root.path().join(".fixture-1.lock");
    fs::write(&lock, "pid=0\n").unwrap();
    assert!(install(root.path(), &plan, |_| panic!("locked"), accept).is_err());
    assert_eq!(fs::read_to_string(lock).unwrap(), "pid=0\n");
}

#[test]
fn rejects_store_symlinks_world_writable_roots_and_installed_hardlinks() {
    let (plan, bytes) = fixture();
    let root = private_temp();
    let link = root.path().join("link");
    symlink(root.path(), &link).unwrap();
    assert!(install(&link, &plan, |_| panic!("unsafe store"), accept).is_err());
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o777)).unwrap();
    assert!(install(root.path(), &plan, |_| panic!("unsafe store"), accept).is_err());
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
    fs::remove_file(link).unwrap();
    let target = install(root.path(), &plan, fetch(bytes), accept).unwrap();
    fs::hard_link(
        target.path().join("primary/sub/data.bin"),
        root.path().join("extra-link"),
    )
    .unwrap();
    assert!(verify(target.path(), &plan, accept).is_err());
}

#[test]
fn publication_never_overwrites_even_an_empty_racing_destination() {
    let (plan, bytes) = fixture();
    let root = private_temp();
    let target = root.path().join(&plan.version);
    assert!(install(root.path(), &plan, fetch(bytes), |_| {
        fs::create_dir(&target)
    })
    .is_err());
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn malformed_plans_are_rejected_before_store_creation() {
    let (plan, _) = fixture();
    for variant in 0..8 {
        let mut bad = plan.clone();
        match variant {
            0 => bad.version = "../escape".into(),
            1 => bad.archives[0].component = "../escape".into(),
            2 => bad.archives[0].sha256 = "0".into(),
            3 => bad.archives[0].files[0].path = "../escape".into(),
            4 => {
                let file = bad.archives[0].files[0].clone();
                bad.archives[0].files.push(file);
            }
            5 => bad.archives.push(bad.archives[0].clone()),
            6 => bad.archives[0].files[0].bytes = u64::MAX,
            7 => bad.archives[0].files.push(FilePin {
                path: "sub".into(),
                bytes: 0,
                sha256: sha256_hex(b""),
            }),
            _ => unreachable!(),
        }
        let root = private_temp();
        let missing = root.path().join("new-store");
        assert!(install(&missing, &bad, |_| panic!("invalid plan"), accept).is_err());
        assert!(!missing.exists());
    }
}

#[test]
fn simulated_disk_write_failure_propagates_without_accepting_a_partial_stream() {
    struct Full;
    impl Write for Full {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::from_raw_os_error(libc::ENOSPC))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let error = copy_pinned(
        &mut Cursor::new(b"payload"),
        &mut Full,
        7,
        &sha256_hex(b"payload"),
    )
    .unwrap_err();
    assert_eq!(error.raw_os_error(), Some(libc::ENOSPC));
}

#[test]
fn killed_installer_leaves_an_inspectable_lock_and_never_exposes_partial_release() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let outer = private_temp();
    let store = outer.path().join("store");
    let ready = outer.path().join("ready");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "cli::database::install::tests::interruption_worker",
            "--nocapture",
        ])
        .env("PTERO_INSTALL_TEST_STORE", &store)
        .env("PTERO_INSTALL_TEST_READY", &ready)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready.exists() && Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            panic!("worker exited before staging");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let staged = ready.exists();
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(staged, "worker did not reach staging");
    assert!(!store.join("fixture-1").exists());
    let before: BTreeSet<_> = fs::read_dir(&store)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert!(before.contains(std::ffi::OsStr::new(".fixture-1.lock")));
    assert_eq!(before.len(), 2); // lock and private staging directory
    let (plan, _) = fixture();
    let error = install(
        &store,
        &plan,
        |_| panic!("must preserve interrupted lock"),
        accept,
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    let after: BTreeSet<_> = fs::read_dir(&store)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(before, after);
}

#[test]
fn interruption_worker() {
    let Some(store) = std::env::var_os("PTERO_INSTALL_TEST_STORE") else {
        return;
    };
    let ready = std::env::var_os("PTERO_INSTALL_TEST_READY").unwrap();
    let (plan, _) = fixture();
    install(
        Path::new(&store),
        &plan,
        |_| {
            fs::write(&ready, b"staging started")?;
            loop {
                std::thread::park();
            }
        },
        accept,
    )
    .unwrap();
}

#[test]
fn exact_archive_root_is_stripped_and_sibling_or_similar_roots_are_refused() {
    let (mut plan, _) = fixture();
    plan.archives[0].archive_root = Some("primary".into());
    for path in [
        "primary/sub/data.bin",
        "primary2/sub/data.bin",
        "other/sub/data.bin",
        "sub/data.bin",
    ] {
        let bytes = gzip(&raw_tar(&[
            ("primary/", b'5', b""),
            (path, b'0', b"pinned fixture payload"),
        ]));
        pinned(&mut plan, &bytes);
        let root = private_temp();
        let result = install(root.path(), &plan, fetch(bytes), accept);
        if path == "primary/sub/data.bin" {
            result.unwrap();
        } else {
            assert!(result.is_err(), "accepted {path}");
            clean(root.path());
        }
    }
}
