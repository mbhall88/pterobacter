//! The committed v2 fixture with real skani databases for both components, so the whole-assembly
//! workflow can measure a synthetic query natively. Every sequence comes from `common::dna`,
//! mutated by a fixed xorshift stream; the package records are rebound exactly as a rebuilt
//! package would be. See `tests/fixtures/reference-v2/README.md`.
use std::collections::BTreeMap;
use std::io::Read;

use serde_json::{json, Value};
use skani::{params::SketchParams, sketch_db::SketchDbWriter, types::Sketch};

use super::package::{sha256_hex, Synthetic, EXPORT_PATH, LICENCE_PATH, SOURCE_PATH};

/// The synthetic base sequence every reference is derived from.
pub fn base() -> Vec<u8> {
    super::dna(200_003, 42)
}

/// `sequence` with about `percent` of its bases substituted, by a fixed xorshift stream.
pub fn mutated(sequence: &[u8], percent: u64, seed: u64) -> Vec<u8> {
    let mut state = seed;
    sequence
        .iter()
        .map(|&base| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            if state % 100 < percent {
                match base {
                    b'A' => b'C',
                    b'C' => b'G',
                    b'G' => b'T',
                    _ => b'A',
                }
            } else {
                base
            }
        })
        .collect()
}

fn u64_at(cursor: &mut std::io::Cursor<Vec<u8>>) -> u64 {
    let mut bytes = [0; 8];
    cursor.read_exact(&mut bytes).unwrap();
    u64::from_le_bytes(bytes)
}

fn replace(value: &mut Value, replacements: &BTreeMap<String, Value>) {
    if let Some(replacement) = value["path"].as_str().and_then(|p| replacements.get(p)) {
        if value.get("sha256").is_some() {
            *value = replacement.clone();
            return;
        }
    }
    match value {
        Value::Object(map) => map.values_mut().for_each(|v| replace(v, replacements)),
        Value::Array(array) => array.iter_mut().for_each(|v| replace(v, replacements)),
        _ => {}
    }
}

/// Write the package with native databases built from `sequence(reference_id)` for every
/// reference of both components. Returns the directory holding `package/` and its manifest pin.
pub fn package(sequence: impl Fn(&str) -> Vec<u8>) -> (tempfile::TempDir, String) {
    let mut package = Synthetic::new();
    let verified = package.verify().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let params = SketchParams::new(1000, 125, 15, false, false);
    let mut replacements = BTreeMap::new();
    let mut entries = BTreeMap::new();
    for component in verified.components() {
        let database = dir.path().join(&component.id);
        std::fs::create_dir(&database).unwrap();
        let mut markers = Vec::new();
        let mut writer = SketchDbWriter::new(database.to_str().unwrap()).unwrap();
        for reference in &component.references {
            let seq = sequence(&reference.reference_id);
            let mut sketch = Sketch::new(1000, 125, 15, reference.search_label.clone(), false);
            sketch.contigs.push("synthetic".into());
            sketch.contig_lengths.push(seq.len() as u32);
            sketch.total_sequence_length = seq.len();
            skani::seeding::fmh_seeds(&seq, &params, 0, &mut sketch, true);
            writer.add_sketch(&params, &sketch).unwrap();
            markers.push(Sketch::get_markers_only(&sketch));
        }
        writer.finalize(database.to_str().unwrap()).unwrap();
        std::fs::write(
            database.join("markers.bin"),
            bincode::serialize(&(params.clone(), markers)).unwrap(),
        )
        .unwrap();
        for file in ["index.db", "sketches.db", "markers.bin"] {
            let asset = &component.databases[file];
            let bytes = std::fs::read(database.join(file)).unwrap();
            replacements.insert(
                asset.source_path.clone(),
                json!({"path": asset.source_path, "bytes": bytes.len(), "sha256": sha256_hex(&bytes)}),
            );
            package.rewrite(&asset.path, bytes);
        }
        // Index records use bincode's pinned u64 lengths and little-endian offsets.
        let mut cursor = std::io::Cursor::new(std::fs::read(database.join("index.db")).unwrap());
        let mut listed = Vec::new();
        for _ in 0..u64_at(&mut cursor) {
            let mut label = vec![0; u64_at(&mut cursor) as usize];
            cursor.read_exact(&mut label).unwrap();
            let offset = u64_at(&mut cursor);
            let bytes = u64_at(&mut cursor);
            listed.push(json!({"path": String::from_utf8(label).unwrap(), "offset": offset, "bytes": bytes}));
        }
        listed.sort_by_key(|entry| entry["path"].as_str().unwrap().to_owned());
        entries.insert(component.id.clone(), listed);
    }
    package.edit_json(SOURCE_PATH, |source| {
        replace(source, &replacements);
        for component in source["components"].as_array_mut().unwrap() {
            let id = component["component_id"].as_str().unwrap().to_owned();
            component["database_index_entries"] = json!(entries[&id]);
        }
    });
    let source_hash = sha256_hex(package.file(SOURCE_PATH));
    let database_bytes: usize = package
        .files()
        .iter()
        .filter(|(p, _)| p.starts_with("databases/"))
        .map(|(_, b)| b.len())
        .sum();
    package.edit_json(EXPORT_PATH, |export| {
        export["source_manifest_sha256"] = source_hash.clone().into();
        export["local_validation"]["database_bytes"] = database_bytes.into();
    });
    package.edit_json(LICENCE_PATH, |licence| {
        licence["source_manifest_sha256"] = source_hash.clone().into();
    });
    package.verify().unwrap();
    package.write_to(&dir.path().join("package"));
    (dir, package.manifest_sha256())
}
