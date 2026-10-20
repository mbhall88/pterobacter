use std::fs;
use terobacter::embedded_skani::params::SketchParams;
use terobacter::embedded_skani::sketch_db::{SketchDbReader, SketchDbWriter};
use terobacter::embedded_skani::types::{SeedPosition, Sketch, TaggedIndex};
use terobacter::skani::{SearchSettings, SeedingMode, SkaniDatabase};
mod common;

fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let params = SketchParams::new(1000, 125, 15, false, false);
    let seq = common::dna(200_003, 42);
    let query = dir.path().join("query.fa");
    fs::write(&query, [b">synthetic\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    let mut writer = SketchDbWriter::new(dir.path().to_str().unwrap()).unwrap();
    let mut markers = Vec::new();
    for id in ["ref-b", "ref-a"] {
        let mut sketch = Sketch::new(1000, 125, 15, id.into(), false);
        sketch.contigs.push("synthetic".into());
        sketch.contig_lengths.push(seq.len() as u32);
        sketch.total_sequence_length = seq.len();
        terobacter::embedded_skani::seeding::fmh_seeds(&seq, &params, 0, &mut sketch, true);
        writer.add_sketch(&params, &sketch).unwrap();
        markers.push(Sketch::get_markers_only(&sketch));
    }
    writer.finalize(dir.path().to_str().unwrap()).unwrap();
    fs::write(
        dir.path().join("markers.bin"),
        bincode::serialize(&(params, markers)).unwrap(),
    )
    .unwrap();
    (dir, query)
}

#[test]
fn emitted_values_and_ties_are_deterministic_across_threads_and_paths() {
    let (dir, query) = fixture();
    let db = SkaniDatabase::open(dir.path()).unwrap();
    let expected = db
        .search(&query, SearchSettings::Primary, 1, SeedingMode::Portable)
        .unwrap();
    assert_eq!(expected.rows.len(), 2);
    assert_eq!(expected.rows[0].reference_file, "ref-a");
    assert_eq!(expected.rows[1].reference_file, "ref-b");
    assert_eq!(expected.rows[0].ani.to_string(), "100.00");
    for threads in [1, 2, 8] {
        for mode in [SeedingMode::Auto, SeedingMode::Portable] {
            let report = db
                .search(&query, SearchSettings::Primary, threads, mode)
                .unwrap();
            assert_eq!(
                serde_json::to_string(&report).unwrap(),
                serde_json::to_string(&expected).unwrap()
            );
        }
    }
}

#[test]
fn concurrent_searches_keep_thread_and_seeding_choices_local() {
    let (dir, query) = fixture();
    let db = SkaniDatabase::open(dir.path()).unwrap();
    std::thread::scope(|scope| {
        let a = scope.spawn(|| {
            db.search(&query, SearchSettings::Primary, 1, SeedingMode::Portable)
                .unwrap()
        });
        let b = scope.spawn(|| {
            db.search(&query, SearchSettings::Primary, 2, SeedingMode::Auto)
                .unwrap()
        });
        assert_eq!(a.join().unwrap(), b.join().unwrap());
    });
}

#[test]
fn genomovar_keeps_low_af_rows_that_primary_reporting_floor_omits() {
    let (dir, query) = fixture();
    let db = SkaniDatabase::open(dir.path()).unwrap();
    let mut fasta = fs::read(&query).unwrap();
    // Keep 20 kb shared; replace the rest with independently generated DNA.
    fasta[11 + 20_000..11 + 200_003].copy_from_slice(&common::dna(180_003, 7654321));
    fs::write(&query, fasta).unwrap();
    let primary = db
        .search(&query, SearchSettings::Primary, 2, SeedingMode::Portable)
        .unwrap();
    let genomovar = db
        .search(&query, SearchSettings::Genomovar, 2, SeedingMode::Portable)
        .unwrap();
    assert!(primary.rows.is_empty());
    assert_eq!(genomovar.rows.len(), 2);
    assert!(genomovar
        .rows
        .iter()
        .all(|r| r.af_reference.as_f64() < 15.0 && r.af_query.as_f64() < 15.0));
}

#[test]
fn bad_assemblies_and_threads_are_errors_while_short_contigs_return_no_estimate() {
    let (dir, query) = fixture();
    let db = SkaniDatabase::open(dir.path()).unwrap();
    assert!(db
        .search(&query, SearchSettings::Primary, 0, SeedingMode::Auto)
        .is_err());
    for bad in [
        b"".as_slice(),
        b"not fasta",
        b">x\nACT$\n",
        b"@x\nACGT\n+\n!!!!\n",
        b">x\nACGT\n>bad\nAC$T\n",
    ] {
        fs::write(&query, bad).unwrap();
        assert!(db
            .search(&query, SearchSettings::Primary, 1, SeedingMode::Auto)
            .is_err());
    }
    fs::write(&query, b">short\nACGT\n").unwrap();
    let short = db
        .search(&query, SearchSettings::Primary, 1, SeedingMode::Auto)
        .unwrap();
    assert!(short.rows.is_empty());
    assert_eq!(short.total_input_bases, 4);
    assert_eq!(short.sketched_bases, 0);
    assert_eq!(short.not_returned_reference_files, ["ref-a", "ref-b"]);
}

#[test]
fn database_corruption_is_an_error_instead_of_a_partial_result() {
    let (dir, _) = fixture();
    fs::write(dir.path().join("sketches.db"), b"truncated").unwrap();
    assert!(SkaniDatabase::open(dir.path()).is_err());
    let (dir, _) = fixture();
    fs::write(dir.path().join("markers.bin"), b"broken").unwrap();
    assert!(SkaniDatabase::open(dir.path()).is_err());
    let (dir, _) = fixture();
    let index = dir.path().join("index.db");
    let mut entries: Vec<terobacter::embedded_skani::sketch_db::IndexEntry> =
        bincode::deserialize(&fs::read(&index).unwrap()).unwrap();
    entries[0].offset = u64::MAX;
    fs::write(index, bincode::serialize(&entries).unwrap()).unwrap();
    assert!(SkaniDatabase::open(dir.path()).is_err());
    let (dir, query) = fixture();
    // Correct bounds, corrupt payload: fail instead of silently dropping a hit.
    let payload = dir.path().join("sketches.db");
    let mut bytes = fs::read(&payload).unwrap();
    bytes[..8].fill(255);
    fs::write(payload, bytes).unwrap();
    assert!(SkaniDatabase::open(dir.path())
        .and_then(|db| db.search(&query, SearchSettings::Primary, 1, SeedingMode::Auto))
        .is_err());
}

#[test]
fn incompatible_marker_profile_is_rejected_without_unwinding() {
    let cases: [fn(&mut Sketch); 4] = [
        |s| s.amino_acid = true,
        |s| s.k = 14,
        |s| s.c = 0,
        |s| s.marker_c = 1000,
    ];
    for mutate in cases {
        let (dir, query) = fixture();
        let path = dir.path().join("markers.bin");
        let (params, mut markers): (SketchParams, Vec<Sketch>) =
            bincode::deserialize(&fs::read(&path).unwrap()).unwrap();
        mutate(&mut markers[0]);
        fs::write(path, bincode::serialize(&(params, markers)).unwrap()).unwrap();
        assert_reference_error_without_unwinding(dir.path(), &query);
    }
}

fn assert_reference_error_without_unwinding(dir: &std::path::Path, query: &std::path::Path) {
    let result = std::panic::catch_unwind(|| {
        SkaniDatabase::open(dir)
            .and_then(|db| db.search(query, SearchSettings::Primary, 2, SeedingMode::Portable))
    });
    let error = result
        .expect("malformed database must not panic")
        .unwrap_err();
    assert!(error.to_string().contains("ref-b"), "{error}");
}

fn mutate_reference(dir: &std::path::Path, mutate: impl FnOnce(&mut Sketch)) {
    let reader = SketchDbReader::new(dir.to_str().unwrap()).unwrap();
    let mut references: Vec<_> = (0..reader.len())
        .map(|i| reader.get_sketch(i).unwrap())
        .collect();
    drop(reader);
    mutate(&mut references[0].1);
    let mut writer = SketchDbWriter::new(dir.to_str().unwrap()).unwrap();
    for (params, sketch) in references {
        writer.add_sketch(&params, &sketch).unwrap();
    }
    writer.finalize(dir.to_str().unwrap()).unwrap();
}

#[test]
fn invalid_packed_seed_storage_is_an_error_without_unwinding_or_partial_results() {
    let (dir, query) = fixture();
    mutate_reference(dir.path(), |sketch| {
        for tag in sketch.kmer_seeds_k.as_mut().unwrap().values_mut() {
            *tag = TaggedIndex::multiple(999_999);
        }
    });
    assert_reference_error_without_unwinding(dir.path(), &query);
}

#[test]
fn invalid_single_and_multiple_seed_positions_are_rejected() {
    for multiple in [false, true] {
        for position in [
            SeedPosition::new(20, 1, false),
            SeedPosition::new(200_003, 0, true),
            SeedPosition::new(0, 0, false),
            SeedPosition {
                pos: 20,
                contig_index_canonical: 0x8000_0000,
            },
        ] {
            // Single tags encode only 31 bits of contig/canonical metadata;
            // the reserved high bit is representable only in stored positions.
            if !multiple && position.contig_index_canonical & 0x8000_0000 != 0 {
                continue;
            }
            let (dir, query) = fixture();
            mutate_reference(dir.path(), |sketch| {
                let tag = if multiple {
                    sketch.multi_position_storage.push(
                        [SeedPosition::new(20, 0, false), position.clone()]
                            .as_slice()
                            .into(),
                    );
                    TaggedIndex::multiple(sketch.multi_position_storage.len() - 1)
                } else {
                    TaggedIndex::single(&position)
                };
                for value in sketch.kmer_seeds_k.as_mut().unwrap().values_mut() {
                    *value = tag;
                }
            });
            assert_reference_error_without_unwinding(dir.path(), &query);
        }
    }
}

#[test]
fn inconsistent_reference_metadata_is_rejected_even_when_query_would_not_screen() {
    let cases: [fn(&mut Sketch); 10] = [
        |s| s.amino_acid = true,
        |s| s.marker_c = 1000,
        |s| s.total_sequence_length += 1,
        |s| s.contigs.clear(),
        |s| s.contig_lengths.clear(),
        |s| s.contig_lengths[0] = 0,
        |s| s.contig_lengths[0] += 1,
        |s| s.kmer_seeds_k = None,
        |s| s.multi_position_storage.push(Default::default()),
        |s| {
            s.multi_position_storage
                .push([SeedPosition::new(20, 0, false)].as_slice().into());
        },
    ];
    for mutate in cases {
        let (dir, query) = fixture();
        mutate_reference(dir.path(), mutate);
        fs::write(&query, b">short\nACGT\n").unwrap();
        assert_reference_error_without_unwinding(dir.path(), &query);
    }
}
