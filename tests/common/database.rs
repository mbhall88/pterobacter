//! DC-1 archives of the existing synthetic native/typing fixtures, generated inside each test.
use super::package::sha256_hex;
use flate2::{write::GzEncoder, Compression};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};
pub fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(base, &p, out)
            } else {
                out.insert(
                    p.strip_prefix(base).unwrap().to_str().unwrap().into(),
                    fs::read(p).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    files
}
pub fn archive(
    id: &str,
    files: BTreeMap<String, Vec<u8>>,
    manifest: &str,
    need: &str,
    binds: Value,
) -> (Value, Vec<u8>) {
    let mut tar = tar::Builder::new(Vec::new());
    let mut members = vec![];
    let identity = sha256_hex(&files[manifest]);
    let mut size = 0;
    for (p, b) in files {
        let mut h = tar::Header::new_ustar();
        h.set_path(format!("{id}/{p}")).unwrap();
        h.set_size(b.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        tar.append(&h, b.as_slice()).unwrap();
        size += b.len();
        members.push(json!({"path":p,"bytes":b.len(),"sha256":sha256_hex(&b)}));
    }
    let raw = tar.into_inner().unwrap();
    let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
    gz.write_all(&raw).unwrap();
    let b = gz.finish().unwrap();
    (
        json!({"id":id,"need":need,"archive":{"file":format!("{id}.tar.gz"),"bytes":b.len(),"sha256":sha256_hex(&b),"tar_bytes":raw.len(),"urls":[]},"identity":{"manifest":manifest,"sha256":identity},"extracted":{"root":id,"files":members.len(),"bytes":size},"members":members,"binds":binds}),
        b,
    )
}
pub struct Fixture {
    pub source: tempfile::TempDir,
    pub reference: String,
    pub query: PathBuf,
    pub catalogue: Value,
    pub catalogue_path: PathBuf,
    pub catalogue_pin: String,
    pub archive_dir: PathBuf,
}
impl Fixture {
    pub fn new() -> Self {
        let (source, reference, seq) = super::assembly::rescue_package();
        let query = source.path().join("fixed-query.fa");
        fs::write(
            &query,
            [b">synthetic\n".as_slice(), &seq, b"\n>plasmid\nACGTN\n"].concat(),
        )
        .unwrap();
        let archive_dir = source.path().join("archives");
        fs::create_dir(&archive_dir).unwrap();
        let mlst = super::mlst::SyntheticScheme::new();
        let parts = [
            archive(
                "reference",
                tree(&source.path().join("package")),
                "portable.json",
                "required",
                json!({}),
            ),
            archive(
                "primary-fasta",
                tree(&source.path().join("fastas")),
                "anim.json",
                "default_workflow",
                json!({"reference":reference}),
            ),
            archive(
                "mlst-historical",
                mlst.files,
                "manifest.json",
                "optional",
                json!({}),
            ),
        ];
        for (c, b) in &parts {
            fs::write(archive_dir.join(c["archive"]["file"].as_str().unwrap()), b).unwrap();
        }
        let catalogue = json!({"schema":"pterobacter.database-catalogue/1","fixture":"Generated synthetic software test; not a published database.","releases":[{"database":"synthetic","version":"fixture-1","status":"candidate","default":false,"schemas":{"reference":"2.0.0-dev-portable-reference-package","primary-fasta":"terobacter.anim-reference/1","mlst-historical":1},"citation":{"doi":null,"concept_doi":null,"text":"synthetic"},"components":parts.into_iter().map(|(c,_)|c).collect::<Vec<_>>()}]});
        let catalogue_path = source.path().join("catalogue.json");
        let mut f = Self {
            source,
            reference,
            query,
            catalogue,
            catalogue_path,
            catalogue_pin: String::new(),
            archive_dir,
        };
        f.save();
        f
    }
    pub fn save(&mut self) {
        let b = serde_json::to_vec_pretty(&self.catalogue).unwrap();
        self.catalogue_pin = sha256_hex(&b);
        fs::write(&self.catalogue_path, b).unwrap();
    }
}
