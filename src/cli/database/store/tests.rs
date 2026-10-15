use super::*;
use crate::cli::database::catalogue::*;
use flate2::{write::GzEncoder, Compression};
use std::{collections::BTreeMap, io::Cursor, os::unix::fs::PermissionsExt};
fn setup() -> (tempfile::TempDir, Release, Vec<u8>) {
    let temp = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let mut tar = tar::Builder::new(Vec::new());
    let mut h = tar::Header::new_ustar();
    h.set_path("reference/portable.json").unwrap();
    h.set_size(2);
    h.set_mode(0o600);
    h.set_cksum();
    tar.append(&h, b"{}".as_slice()).unwrap();
    let raw = tar.into_inner().unwrap();
    let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
    gzip.write_all(&raw).unwrap();
    let data = gzip.finish().unwrap();
    let hash = crate::evidence::sha256_hex;
    let c = Component {
        id: "reference".into(),
        need: "required".into(),
        archive: Archive {
            file: "reference.tar.gz".into(),
            bytes: data.len() as u64,
            sha256: hash(&data),
            tar_bytes: raw.len() as u64,
            urls: vec![],
        },
        extracted: Extracted {
            root: "reference".into(),
            files: 1,
            bytes: 2,
        },
        identity: Identity {
            manifest: "portable.json".into(),
            sha256: hash(b"{}"),
        },
        binds: BTreeMap::new(),
        members: vec![install::FilePin {
            path: "portable.json".into(),
            bytes: 2,
            sha256: hash(b"{}"),
        }],
    };
    (
        temp,
        Release {
            database: "test".into(),
            version: "1".into(),
            status: "candidate".into(),
            default: false,
            schemas: BTreeMap::new(),
            citation: serde_json::Value::Null,
            components: vec![c],
        },
        data,
    )
}
#[test]
fn activation_failure_retains_component_and_retry_reconstructs_receipt() {
    let (temp, r, bytes) = setup();
    let store = Store::open(temp.path(), &r).unwrap();
    let c = &r.components[0];
    let err = store
        .install_with(
            c,
            || Ok(Box::new(Cursor::new(bytes))),
            |_| Ok(()),
            |dir| File::open(dir)?.sync_all(),
            |_| Err(io::Error::other("receipt interrupted")),
        )
        .unwrap_err();
    assert!(err.activated);
    assert!(store.version.join("reference/portable.json").exists());
    assert_eq!(
        store
            .install(c, "pin", || panic!("offline reuse"), |_| Ok(()))
            .unwrap(),
        "verified_reuse"
    );
    assert!(store.version.join("receipts/reference.json").is_file());
}
#[test]
fn later_optional_activation_preserves_reference_and_uses_one_version_lock() {
    let (temp, r, bytes) = setup();
    let store = Store::open(temp.path(), &r).unwrap();
    assert!(Store::open(temp.path(), &r).is_err());
    store
        .install(
            &r.components[0],
            "pin",
            || Ok(Box::new(Cursor::new(bytes))),
            |_| Ok(()),
        )
        .unwrap();
    let before = fs::read(store.version.join("reference/portable.json")).unwrap();
    // A failed later component must not remove the already activated reference.
    let mut other = r.components[0].clone();
    other.id = "mlst-historical".into();
    other.extracted.root = other.id.clone();
    assert!(store
        .install(
            &other,
            "pin",
            || Err(io::Error::other("cancelled")),
            |_| Ok(())
        )
        .is_err());
    assert_eq!(
        before,
        fs::read(store.version.join("reference/portable.json")).unwrap()
    );
    drop(store);
    assert!(Store::open(temp.path(), &r).is_ok());
}

#[test]
fn parent_sync_failure_retains_activation_and_receipt_does_not_authorize_reuse() {
    let (temp, r, bytes) = setup();
    let store = Store::open(temp.path(), &r).unwrap();
    let c = &r.components[0];
    let error = store
        .install_with(
            c,
            || Ok(Box::new(Cursor::new(bytes))),
            |_| Ok(()),
            |_| Err(io::Error::other("parent sync interrupted")),
            |_| panic!("no receipt before durability"),
        )
        .unwrap_err();
    assert!(error.activated);
    assert!(store.version.join("reference/portable.json").is_file());
    store
        .install(c, "pin", || panic!("offline"), |_| Ok(()))
        .unwrap();
    fs::write(
        store.version.join("receipts/reference.json"),
        b"untrusted receipt",
    )
    .unwrap();
    fs::write(store.version.join("reference/portable.json"), b"corruption").unwrap();
    assert!(store
        .install(
            c,
            "pin",
            || panic!("corruption must not fetch"),
            |_| panic!("hash first")
        )
        .is_err());
}
#[test]
fn killed_component_installer_preserves_abandoned_lock_and_stage() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let (temp, r, _) = setup();
    let root = temp.path().join("store");
    let ready = temp.path().join("ready");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "cli::database::store::tests::interruption_worker",
            "--nocapture",
        ])
        .env("PTERO_COMPONENT_TEST_ROOT", &root)
        .env("PTERO_COMPONENT_TEST_READY", &ready)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready.exists() && Instant::now() < deadline {
        assert!(
            child.try_wait().unwrap().is_none(),
            "worker exited before staging"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let staged = ready.exists();
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(staged);
    let base = root.join("test/1");
    let before: Vec<_> = fs::read_dir(&base)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(before.len(), 1);
    assert!(!base.join("reference").exists());
    assert!(root.join("test/.1.lock").is_file());
    assert!(Store::open(&root, &r).is_err());
    let after: Vec<_> = fs::read_dir(base)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(before, after);
}
#[test]
fn interruption_worker() {
    let Some(root) = std::env::var_os("PTERO_COMPONENT_TEST_ROOT") else {
        return;
    };
    let ready = std::env::var_os("PTERO_COMPONENT_TEST_READY").unwrap();
    let (_temp, r, _) = setup();
    let store = Store::open(Path::new(&root), &r).unwrap();
    let _ = store.install(
        &r.components[0],
        "pin",
        || {
            fs::write(ready, b"staging")?;
            loop {
                std::thread::park();
            }
        },
        |_| Ok(()),
    );
}

#[test]
fn simultaneous_component_installers_share_one_version_lock() {
    use std::sync::Barrier;
    let (temp, r, _) = setup();
    let start = Barrier::new(2);
    let held = Barrier::new(2);
    let results = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..2)
            .map(|_| {
                scope.spawn(|| {
                    start.wait();
                    let store = Store::open(temp.path(), &r);
                    held.wait();
                    store.is_ok()
                })
            })
            .collect();
        workers
            .into_iter()
            .filter(|w| w.thread().id() != std::thread::current().id())
            .map(|w| w.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(results.into_iter().filter(|x| *x).count(), 1);
}
