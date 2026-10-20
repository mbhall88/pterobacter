//! Native synthetic package builder derived from the frozen reference-v2 fixture.
use serde_json::Value;
pub fn native_package() -> (tempfile::TempDir, String, Vec<u8>) {
    build_package(false, NativeCase::Rescue)
}

pub fn rescue_package() -> (tempfile::TempDir, String, Vec<u8>) {
    build_package(true, NativeCase::Rescue)
}

#[derive(Clone, Copy, PartialEq)]
pub enum NativeCase {
    Rescue,
    ForeignChallenge,
    MalformedReference,
}
pub fn native_case(case: NativeCase) -> (tempfile::TempDir, String, Vec<u8>) {
    build_package(true, case)
}

fn build_package(rescue: bool, case: NativeCase) -> (tempfile::TempDir, String, Vec<u8>) {
    use super::package::{sha256_hex, Synthetic, EXPORT_PATH, SOURCE_PATH};
    use serde_json::json;
    use terobacter::embedded_skani::{
        params::SketchParams, sketch_db::SketchDbWriter, types::Sketch,
    };
    let mut package = Synthetic::new();
    let verified = package.verify().unwrap();
    let component = verified.component("primary_whole").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("database");
    std::fs::create_dir(&database).unwrap();
    let params = SketchParams::new(1000, 125, 15, false, false);
    let original = super::dna(200_003, 42);
    let mut fastas = std::collections::BTreeMap::new();
    let mut markers = Vec::new();
    let mut writer = SketchDbWriter::new(database.to_str().unwrap()).unwrap();
    for (i, reference) in component.references.iter().enumerate() {
        let mut seq = original.clone();
        if i > 0 {
            let mut state = 927_u64;
            for base in &mut seq {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                if state % 100 < 8 {
                    *base = match *base {
                        b'A' => b'C',
                        b'C' => b'G',
                        b'G' => b'T',
                        _ => b'A',
                    };
                }
            }
        }
        if rescue && i == 0 {
            for base in seq.iter_mut().step_by(32) {
                *base = if *base == b'A' { b'C' } else { b'A' };
            }
        }
        if rescue && i == 4 {
            seq = super::dna(original.len(), 7654321);
        }
        if rescue && i == 3 && case == NativeCase::ForeignChallenge {
            seq = original.clone();
            for base in seq.iter_mut().step_by(31) {
                *base = if *base == b'A' { b'C' } else { b'A' };
            }
        }
        let mut fasta = [b">synthetic\n".as_slice(), &seq, b"\n"].concat();
        if rescue && i == 0 && case == NativeCase::MalformedReference {
            fasta.extend_from_slice(b">synthetic\nACGT\n");
        }
        fastas.insert(reference.reference_id.clone(), fasta);
        let mut sketch = Sketch::new(1000, 125, 15, reference.search_label.clone(), false);
        sketch.contigs.push("synthetic".into());
        sketch.contig_lengths.push(seq.len() as u32);
        sketch.total_sequence_length = seq.len();
        terobacter::embedded_skani::seeding::fmh_seeds(&seq, &params, 0, &mut sketch, true);
        writer.add_sketch(&params, &sketch).unwrap();
        markers.push(Sketch::get_markers_only(&sketch));
    }
    writer.finalize(database.to_str().unwrap()).unwrap();
    std::fs::write(
        database.join("markers.bin"),
        bincode::serialize(&(params, markers)).unwrap(),
    )
    .unwrap();
    let mut replacements = std::collections::BTreeMap::new();
    for file in ["index.db", "sketches.db", "markers.bin"] {
        let asset = &component.databases[file];
        let bytes = std::fs::read(database.join(file)).unwrap();
        replacements.insert(
            asset.source_path.clone(),
            json!({"path": asset.source_path,
            "bytes": bytes.len(), "sha256": sha256_hex(&bytes)}),
        );
        package.rewrite(&asset.path, bytes);
    }
    if rescue {
        let table_path = "metadata/tables/primary_references.tsv";
        let mut table = String::from_utf8(package.file(table_path).to_vec()).unwrap();
        for r in &component.references {
            let bytes = &fastas[&r.reference_id];
            let digest = sha256_hex(bytes);
            table = table.replace(&r.source.sha256, &digest);
            replacements.insert(
                r.source.label.clone(),
                json!({"path": r.source.label, "bytes": bytes.len(), "sha256": digest}),
            );
        }
        package.rewrite(table_path, table.into_bytes());
        let manifest: Value = serde_json::from_slice(&package.files()["portable.json"]).unwrap();
        let asset = manifest["assets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["path"] == table_path)
            .unwrap();
        let source_path = asset["source_path"].as_str().unwrap();
        replacements.insert(source_path.to_owned(), json!({"path": source_path, "bytes": package.file(table_path).len(), "sha256": sha256_hex(package.file(table_path))}));
    }
    // Index records use bincode's pinned u64 lengths and little-endian offsets.
    let bytes = std::fs::read(database.join("index.db")).unwrap();
    let mut cursor = std::io::Cursor::new(bytes);
    use std::io::Read;
    fn u64_at(cursor: &mut std::io::Cursor<Vec<u8>>) -> u64 {
        let mut bytes = [0; 8];
        cursor.read_exact(&mut bytes).unwrap();
        u64::from_le_bytes(bytes)
    }
    let count = u64_at(&mut cursor);
    let mut entries = Vec::new();
    for _ in 0..count {
        let size = u64_at(&mut cursor);
        let mut label = vec![0; size as usize];
        cursor.read_exact(&mut label).unwrap();
        let offset = u64_at(&mut cursor);
        let size = u64_at(&mut cursor);
        entries.push(
            json!({"path": String::from_utf8(label).unwrap(), "offset": offset, "bytes": size}),
        );
    }
    entries.sort_by_key(|entry| entry["path"].as_str().unwrap().to_owned());
    fn replace(value: &mut Value, replacements: &std::collections::BTreeMap<String, Value>) {
        if let Some(replacement) = value["path"].as_str().and_then(|p| replacements.get(p)) {
            if value.get("sha256").is_some() {
                *value = replacement.clone();
                return;
            }
        }
        match value {
            Value::Object(map) => {
                for child in map.values_mut() {
                    replace(child, replacements);
                }
            }
            Value::Array(array) => {
                for child in array {
                    replace(child, replacements);
                }
            }
            _ => {}
        }
    }
    package.edit_json(SOURCE_PATH, |source| {
        replace(source, &replacements);
        let primary = source["components"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|c| c["component_id"] == "primary_whole")
            .unwrap();
        primary["database_index_entries"] = json!(entries);
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
    package.edit_json(super::package::LICENCE_PATH, |licence| {
        licence["source_manifest_sha256"] = source_hash.clone().into();
    });
    package.verify().unwrap();
    let target = dir.path().join("package");
    package.write_to(&target);
    let pin = package.manifest_sha256();
    if rescue {
        let component_dir = dir.path().join("fastas");
        std::fs::create_dir_all(component_dir.join("sequences")).unwrap();
        let verified = package.verify().unwrap();
        let mut references = Vec::new();
        for (i, r) in verified
            .component("primary_whole")
            .unwrap()
            .references
            .iter()
            .enumerate()
        {
            let path = format!("sequences/{i}.fa");
            std::fs::write(component_dir.join(&path), &fastas[&r.reference_id]).unwrap();
            let mut concepts = vec![r.species_id.clone()];
            if let Some(id) = &r.subspecies_id {
                concepts.push(id.clone());
            }
            references.push(
                json!({"reference_id": r.reference_id, "path": path, "sha256": r.source.sha256,
                "bytes": r.source.bytes, "concept_ids": concepts}),
            );
        }
        let licence = package.file(super::package::LICENCE_PATH);
        std::fs::write(component_dir.join("licence.json"), licence).unwrap();
        let manifest = json!({"schema": "terobacter.anim-reference/1", "component_id": "primary_whole",
            "sequence_scope": "whole_assembly_as_deposited", "reference_package_sha256": pin,
            "licence": {"path": "licence.json", "sha256": sha256_hex(licence), "bytes": licence.len()},
            "references": references});
        std::fs::write(
            component_dir.join("anim.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
    }
    (dir, pin, original)
}
