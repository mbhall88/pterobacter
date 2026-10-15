//! Public database commands and offline classification resolution.
use super::{
    catalogue::{self, invalid, Catalogue, Component, Release},
    install,
    store::Store,
};
use crate::cli::Directory;
use crate::reference::{
    anim::verify_anim_reference, verify_package, RegistryPins, VerifiedPackage,
};
use clap::{Args, Subcommand};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Default, Args)]
pub struct Options {
    /// Exact installed database version; otherwise use the catalogue's published default
    #[arg(short = 'D', long, global = true)]
    pub database: Option<String>,
    /// Managed database store on a filesystem supporting atomic no-replace rename
    #[arg(short = 'B', long, global = true, value_name = "DIR")]
    pub database_dir: Option<PathBuf>,
    /// Explicit trusted catalogue file; requires its independently supplied SHA-256
    #[arg(short = 'C', long, global = true, requires = "catalogue_sha256")]
    pub catalogue: Option<PathBuf>,
    /// Expected SHA-256 of the explicit catalogue
    #[arg(short='H',long,global=true,requires="catalogue",value_parser=super::super::parse_sha256)]
    pub catalogue_sha256: Option<String>,
}
#[derive(Debug, Subcommand)]
pub enum Database {
    /// Install, list or verify pinned reference and typing components
    Database {
        #[command(subcommand)]
        action: Action,
    },
}
#[derive(Debug, Subcommand)]
pub enum Action {
    /// Install an exact version; reference and primary FASTAs are included by default
    Install {
        version: String,
        /// Directory containing locally supplied component archives; no network use
        #[arg(short = 'q', long, conflicts_with = "from_file")]
        from_directory: Option<PathBuf>,
        /// Local archive file, repeat for multiple components; no network use
        #[arg(short = 'f', long, conflicts_with = "from_directory")]
        from_file: Vec<PathBuf>,
        /// Also install the release's optional typing component
        #[arg(short = 'l', long)]
        mlst: bool,
        /// Install only components needed for skani-only classification
        #[arg(short = 'n', long)]
        no_anim: bool,
    },
    /// List catalogue versions and local component presence; presence alone is not verification
    List,
    /// Fully verify an exact installed version and its component bindings
    Verify {
        version: String,
        #[arg(short = 'n', long)]
        no_anim: bool,
    },
}
pub struct Selection {
    pub provenance: Value,
    pub typing_error: Option<String>,
}
fn platform_dir(state: bool) -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = home.map(|h| h.join("Library/Application Support"));
    #[cfg(not(target_os = "macos"))]
    let base = std::env::var_os(if state {
        "XDG_STATE_HOME"
    } else {
        "XDG_DATA_HOME"
    })
    .map(PathBuf::from)
    .or_else(|| {
        home.map(|h| {
            h.join(if state {
                ".local/state"
            } else {
                ".local/share"
            })
        })
    });
    #[cfg(target_os = "macos")]
    let _ = state;
    let base = base.ok_or_else(|| {
        invalid("no platform user directory; set --database-dir and an explicit --anim-output")
    })?;
    if !base.is_absolute() {
        return Err(invalid("platform data/state directory must be absolute"));
    }
    Ok(base.join("pterobacter"))
}
pub fn state_root() -> io::Result<PathBuf> {
    Ok(platform_dir(true)?.join("evidence"))
}
fn data_root(options: &Options) -> io::Result<PathBuf> {
    options
        .database_dir
        .clone()
        .map(Ok)
        .unwrap_or_else(|| platform_dir(false).map(|p| p.join("databases")))
}
fn load(options: &Options) -> io::Result<(Catalogue, String)> {
    Catalogue::load(
        options.catalogue.as_deref(),
        options.catalogue_sha256.as_deref(),
    )
}
fn directory(root: &Path, r: &Release) -> PathBuf {
    root.join(&r.database).join(&r.version)
}
fn verify_component(
    base: &Path,
    c: &Component,
    release: &Release,
    reference: Option<&VerifiedPackage>,
) -> io::Result<()> {
    let path = base.join(&c.id);
    install::verify_component(&path, &c.pin())?;
    verify_science(&path, c, &release.schemas[&c.id], reference)?;
    install::verify_component(&path, &c.pin())
}
fn verify_science(
    path: &Path,
    c: &Component,
    expected_schema: &Value,
    reference: Option<&VerifiedPackage>,
) -> io::Result<()> {
    match c.id.as_str() {
        "reference" => {
            let package = verify_package(
                &Directory(path.to_owned()),
                &c.identity.sha256,
                &RegistryPins::default(),
            )
            .map_err(io::Error::other)?;
            if expected_schema != &json!(package.package().schema_version) {
                return Err(invalid("reference schema differs from catalogue"));
            }
        }
        "primary-fasta" => {
            verify_anim_reference(
                &Directory(path.to_owned()),
                &c.identity.sha256,
                reference.ok_or_else(|| invalid("primary FASTAs require a verified reference"))?,
            )
            .map_err(io::Error::other)?;
        }
        _ => {
            let scheme =
                crate::mlst::Scheme::verify(&Directory(path.to_owned()), &c.identity.sha256)
                    .map_err(io::Error::other)?;
            let actual = scheme
                .database()
                .get("schema_version")
                .cloned()
                .unwrap_or(json!(1));
            if &actual != expected_schema {
                return Err(invalid("MLST schema differs from catalogue"));
            }
        }
    }
    Ok(())
}
fn reference(base: &Path, r: &Release) -> io::Result<VerifiedPackage> {
    let c = r.component("reference")?;
    verify_component(base, c, r, None)?;
    verify_package(
        &Directory(base.join("reference")),
        &c.identity.sha256,
        &RegistryPins::default(),
    )
    .map_err(io::Error::other)
}
fn selected(r: &Release, no_anim: bool, mlst: bool) -> io::Result<Vec<&Component>> {
    let mut out = vec![r.component("reference")?];
    if !no_anim {
        out.push(r.component("primary-fasta")?);
    }
    if mlst {
        out.push(
            r.typing().ok_or_else(|| {
                invalid("requested MLST component is unavailable in this release")
            })?,
        );
    }
    Ok(out)
}
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(5)
        .timeout_global(Some(Duration::from_secs(600)))
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_recv_body(Some(Duration::from_secs(300)))
        .build()
        .new_agent()
}
fn fetch(
    c: &Component,
    r: &Release,
    local_dir: Option<&Path>,
    files: &BTreeMap<String, PathBuf>,
) -> io::Result<Box<dyn Read>> {
    if let Some(dir) = local_dir {
        return Ok(Box::new(File::open(dir.join(&c.archive.file))?));
    }
    if !files.is_empty() {
        return Ok(Box::new(File::open(
            files
                .get(&c.archive.file)
                .ok_or_else(|| invalid(format!("missing local archive {}", c.archive.file)))?,
        )?));
    }
    if r.status != "published" {
        return Err(invalid(
            "candidate/withdrawn installation requires local archives; no public URL is assigned",
        ));
    }
    let url = c.archive.urls.first().ok_or_else(|| {
        invalid("no verified published download URL is assigned; use local archives")
    })?;
    catalogue::https(url)?;
    let response = agent().get(url).call().map_err(io::Error::other)?;
    if response.status() != 200 {
        return Err(invalid(format!(
            "download returned status {}",
            response.status()
        )));
    }
    Ok(Box::new(response.into_body().into_reader()))
}
pub fn run(command: Database, options: &Options) -> io::Result<Value> {
    if options.database.is_some() {
        return Err(invalid(
            "use the database command's VERSION argument instead of --database",
        ));
    }
    let (cat, cat_sha) = load(options)?;
    let Database::Database { action } = command;
    let root = data_root(options)?;
    match action {
        Action::List => Ok(
            json!({"catalogue_sha256":cat_sha,"versions":cat.releases.iter().map(|r|json!({"database":r.database,"version":r.version,"status":r.status,"default":r.default,"citation":r.citation,"components":r.components.iter().map(|c|json!({"id":c.id,"manifest_sha256":c.identity.sha256,"state":if directory(&root,r).join(&c.id).exists(){"present_unverified"}else{"not_installed"}})).collect::<Vec<_>>()})).collect::<Vec<_>>() }),
        ),
        Action::Verify { version, no_anim } => {
            let r = cat.select(Some(&version))?;
            let base = directory(&root, r);
            let package = reference(&base, r)?;
            let mut required = selected(r, no_anim, false)?;
            for c in &r.components {
                if c.need == "optional" && base.join(&c.id).exists() {
                    required.push(c);
                }
            }
            for c in required {
                verify_component(&base, c, r, Some(&package))?;
            }
            Ok(
                json!({"status":"verified","database":r.database,"version":r.version,"catalogue_sha256":cat_sha,"citation":r.citation}),
            )
        }
        Action::Install {
            version,
            from_directory,
            from_file,
            mlst,
            no_anim,
        } => {
            let r = cat.select(Some(&version))?;
            let required = selected(r, no_anim, mlst)?;
            if from_directory.as_ref().is_some_and(|p| !p.is_dir()) {
                return Err(invalid("--from-directory must name a local directory"));
            }
            let mut files = BTreeMap::new();
            for p in from_file {
                let name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| invalid("local archive filename is not UTF-8"))?
                    .to_owned();
                if !r.components.iter().any(|c| c.archive.file == name)
                    || files.insert(name, p).is_some()
                {
                    return Err(invalid("unknown or duplicate local archive filename"));
                }
            }
            let store = Store::open(&root, r)?;
            let mut results = vec![];
            for c in required {
                let package = if c.id == "primary-fasta" {
                    Some(reference(&store.version, r)?)
                } else {
                    None
                };
                let state = store
                    .install(
                        c,
                        &cat_sha,
                        || fetch(c, r, from_directory.as_deref(), &files),
                        |path| verify_science(path, c, &r.schemas[&c.id], package.as_ref()),
                    )
                    .map_err(io::Error::other)?;
                results.push(json!({"component":c.id,"status":state,"directory":store.version.join(&c.id),"manifest_sha256":c.identity.sha256}));
            }
            Ok(
                json!({"status":"ready","database":r.database,"version":r.version,"catalogue_sha256":cat_sha,"components":results,"citation":r.citation}),
            )
        }
    }
}
pub fn resolve(args: &mut super::super::Public) -> io::Result<Selection> {
    if args.reference_package.is_some() && args.databases.database.is_some() {
        return Err(invalid("--database conflicts with --reference-package"));
    }
    if args.anim.anim_reference_package.is_some() != args.anim.expected_anim_sha256.is_some() {
        return Err(invalid(
            "--anim-reference-package and --expected-anim-sha256 must be supplied together",
        ));
    }
    let (cat, sha) = load(&args.databases)?;
    let explicit_mlst = args.mlst_package.is_some();
    let explicit_anim = args.anim.anim_reference_package.is_some();
    let catalogue_trust = if args.databases.catalogue.is_some() {
        "operator_pinned_catalogue"
    } else {
        "embedded_catalogue"
    };
    let mut provenance = json!({"reference_trust":"operator_pin","reference_manifest_sha256":args.expected_sha256,"mlst_requested":args.mlst||args.mlst_package.is_some()});
    let mut typing_error = None;
    if args.reference_package.is_none() {
        let r = cat.select(args.databases.database.as_deref())?;
        let root = data_root(&args.databases)?;
        let base = directory(&root, r);
        let package=reference(&base,r).map_err(|e|invalid(format!("selected database is unavailable or corrupt: {e}; install this exact version or supply explicit package paths")))?;
        let c = r.component("reference")?;
        args.reference_package = Some(base.join("reference"));
        args.expected_sha256 = Some(c.identity.sha256.clone());
        if !args.no_anim {
            if let Some(explicit) = &args.anim.anim_reference_package {
                verify_anim_reference(
                    &Directory(explicit.clone()),
                    args.anim
                        .expected_anim_sha256
                        .as_deref()
                        .expect("pair checked"),
                    &package,
                )
                .map_err(io::Error::other)?;
            } else {
                let component = r.component("primary-fasta")?;
                verify_component(&base, component, r, Some(&package))?;
                args.anim.anim_reference_package = Some(base.join("primary-fasta"));
                args.anim.expected_anim_sha256 = Some(component.identity.sha256.clone());
            }
        }
        if args.mlst && args.mlst_package.is_none() {
            match r.typing(){
                None=>typing_error=Some("the selected release has no optional MLST component".into()),
                Some(c)=>match verify_component(&base,c,r,Some(&package)) {
                    Ok(())=>{args.mlst_package=Some(base.join(&c.id));args.mlst_expected_sha256=Some(c.identity.sha256.clone());},
                    Err(e)=>typing_error=Some(format!("requested optional MLST is unavailable: {e}; install this exact component explicitly")),
                }
            }
        }
        provenance = json!({"reference_trust":if args.databases.catalogue.is_some(){"operator_pinned_catalogue"}else{"embedded_catalogue"},"catalogue_sha256":sha,"database":r.database,"version":r.version,"status":r.status,"citation":r.citation,"reference_manifest_sha256":c.identity.sha256});
    } else if args.mlst && args.mlst_package.is_none() {
        typing_error = Some(
            "--mlst needs a selected managed release or an explicit MLST package/digest pair"
                .into(),
        );
    }
    provenance["mlst_requested"] = json!(args.mlst || explicit_mlst);
    if args.mlst || explicit_mlst {
        provenance["mlst_trust"] = json!(if explicit_mlst {
            "operator_pin"
        } else if args.mlst_package.is_some() {
            catalogue_trust
        } else {
            "unavailable"
        });
        provenance["mlst_manifest_sha256"] = json!(args.mlst_expected_sha256);
    }
    if !args.no_anim {
        provenance["anim_trust"] = json!(if explicit_anim {
            "operator_pin"
        } else if args.anim.anim_reference_package.is_some() {
            catalogue_trust
        } else {
            "unavailable"
        });
        provenance["anim_manifest_sha256"] = json!(args.anim.expected_anim_sha256);
    }
    Ok(Selection {
        provenance,
        typing_error,
    })
}

#[cfg(test)]
#[path = "network_tests.rs"]
mod network_tests;
