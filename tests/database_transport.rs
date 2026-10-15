//! Installer-to-existing-CLI seam using the existing frozen synthetic package derivations.
//! This does not define a catalogue format or claim integration with the real public export.
mod common;
use assert_cmd::cargo::cargo_bin_cmd;
use common::package::sha256_hex;
use flate2::{write::GzEncoder, Compression};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Cursor, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use terobacter::cli::database::install::{install, ArchivePin, FilePin, InstallPlan};
use terobacter::reference::{verify_package, PackageSource, RegistryPins};

struct Directory(PathBuf);
impl PackageSource for Directory {
    fn inventory(&self) -> io::Result<std::collections::BTreeSet<String>> {
        Ok(files(&self.0).into_keys().collect())
    }
    fn open(&self, path: &str) -> io::Result<Option<Box<dyn Read + '_>>> {
        match fs::File::open(self.0.join(path)) {
            Ok(file) => Ok(Some(Box::new(file))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
}
fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, directory: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root).unwrap().to_str().unwrap().into(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}
fn archive(component: &str, files: BTreeMap<String, Vec<u8>>) -> (ArchivePin, Vec<u8>) {
    let mut tar = tar::Builder::new(Vec::new());
    let mut pins = Vec::new();
    for (path, bytes) in files {
        let mut header = tar::Header::new_ustar();
        header.set_path(format!("{component}/{path}")).unwrap();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o600);
        header.set_cksum();
        tar.append(&header, bytes.as_slice()).unwrap();
        pins.push(FilePin {
            path,
            bytes: bytes.len() as u64,
            sha256: sha256_hex(&bytes),
        });
    }
    let tar = tar.into_inner().unwrap();
    let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
    gzip.write_all(&tar).unwrap();
    let bytes = gzip.finish().unwrap();
    (
        ArchivePin {
            component: component.into(),
            archive_root: Some(component.into()),
            bytes: bytes.len() as u64,
            sha256: sha256_hex(&bytes),
            tar_bytes_limit: tar.len() as u64,
            files: pins,
        },
        bytes,
    )
}
fn private_temp() -> tempfile::TempDir {
    tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap()
}
fn scientific_verifier(root: &Path, reference: &str, genomes: &str, mlst: &str) -> io::Result<()> {
    let package = verify_package(
        &Directory(root.join("reference")),
        reference,
        &RegistryPins::default(),
    )
    .map_err(io::Error::other)?;
    terobacter::reference::anim::verify_anim_reference(
        &Directory(root.join("genomes")),
        genomes,
        &package,
    )
    .map_err(io::Error::other)?;
    terobacter::mlst::Scheme::verify(&Directory(root.join("mlst")), mlst)
        .map_err(io::Error::other)?;
    Ok(())
}
#[test]
fn local_archive_install_preserves_manifest_identities_and_explicit_cli_scientific_output() {
    let (source, reference, query_bytes) = common::assembly::rescue_package();
    let scheme = common::mlst::SyntheticScheme::new();
    let mlst = sha256_hex(&scheme.files["manifest.json"]);
    let genomes = sha256_hex(&fs::read(source.path().join("fastas/anim.json")).unwrap());
    let components = [
        archive("reference", files(&source.path().join("package"))),
        archive("genomes", files(&source.path().join("fastas"))),
        archive("mlst", scheme.files.clone()),
    ];
    let plan = InstallPlan {
        version: "fixture-seam-1".into(),
        archives: components.iter().map(|(pin, _)| pin.clone()).collect(),
    };
    let store = private_temp();
    let result = install(
        store.path(),
        &plan,
        |pin| {
            let bytes = &components
                .iter()
                .find(|(p, _)| p.component == pin.component)
                .unwrap()
                .1;
            Ok(Box::new(Cursor::new(bytes.clone())))
        },
        |root| scientific_verifier(root, &reference, &genomes, &mlst),
    )
    .unwrap();
    assert_eq!(
        sha256_hex(&fs::read(result.path().join("reference/portable.json")).unwrap()),
        reference
    );
    assert_eq!(
        sha256_hex(&fs::read(result.path().join("genomes/anim.json")).unwrap()),
        genomes
    );
    let explicit_scheme = source.path().join("scheme");
    fs::create_dir(&explicit_scheme).unwrap();
    for (path, bytes) in &scheme.files {
        let target = explicit_scheme.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, bytes).unwrap();
    }
    let query = source.path().join("same-query-label.fa");
    fs::write(
        &query,
        [
            b">synthetic\n".as_slice(),
            &query_bytes,
            b"\n>plasmid\nACGTN\n",
        ]
        .concat(),
    )
    .unwrap();
    let classify = |package: &Path, scheme: &Path| -> Value {
        let output = cargo_bin_cmd!("ptero")
            .arg("--no-anim")
            .arg("--reference-package")
            .arg(package)
            .args(["--expected-sha256", &reference])
            .arg("--mlst-package")
            .arg(scheme)
            .args(["--mlst-expected-sha256", &mlst])
            .arg(&query)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    };
    let explicit = classify(&source.path().join("package"), &explicit_scheme);
    let installed = classify(
        &result.path().join("reference"),
        &result.path().join("mlst"),
    );
    assert_eq!(installed, explicit);
    assert_eq!(installed["input"]["total_bases"], query_bytes.len() + 5);
    assert_eq!(
        installed["mlst"]["database"]["submission_cutoff"],
        "2024-12-31"
    );
    eprintln!(
        "transport seam receipt: {}",
        serde_json::json!({
            "reference_manifest_sha256": reference, "anim_manifest_sha256": genomes,
            "mlst_manifest_sha256": mlst, "report_sha256": sha256_hex(&serde_json::to_vec(&installed).unwrap()),
            "archives": plan.archives.iter().map(|a| serde_json::json!({"component": a.component, "bytes": a.bytes, "sha256": a.sha256})).collect::<Vec<_>>()
        })
    );
    // The replacement passes all transport pins but names a different scientific primary.
    let mut wrong_files = files(&source.path().join("fastas"));
    let mut manifest: Value = serde_json::from_slice(&wrong_files["anim.json"]).unwrap();
    manifest["reference_package_sha256"] = "0".repeat(64).into();
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();
    let wrong_genomes = sha256_hex(&manifest_bytes);
    wrong_files.insert("anim.json".into(), manifest_bytes);
    let mut wrong_components = components.clone();
    wrong_components[1] = archive("genomes", wrong_files);
    let bad_plan = InstallPlan {
        version: "fixture-wrong-binding".into(),
        archives: wrong_components
            .iter()
            .map(|(pin, _)| pin.clone())
            .collect(),
    };
    let bad = install(
        store.path(),
        &bad_plan,
        |pin| {
            let bytes = &wrong_components
                .iter()
                .find(|(p, _)| p.component == pin.component)
                .unwrap()
                .1;
            Ok(Box::new(Cursor::new(bytes.clone())))
        },
        |root| scientific_verifier(root, &reference, &wrong_genomes, &mlst),
    );
    assert!(bad
        .unwrap_err()
        .to_string()
        .contains("does not describe this whole-assembly primary package"));
    assert!(!store.path().join("fixture-wrong-binding").exists());
}
