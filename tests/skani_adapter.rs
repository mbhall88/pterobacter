use skani::params::SketchParams;
use skani::sketch_db::SketchDbWriter;
use skani::types::Sketch;
use std::fs;
use terobacter::skani::{SearchSettings, SeedingMode, SkaniDatabase};

fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let params = SketchParams::new(1000, 125, 15, false, false);
    let mut state = 42_u64;
    let seq: Vec<u8> = (0..200_003)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            b"ACGT"[(state & 3) as usize]
        })
        .collect();
    let query = dir.path().join("query.fa");
    fs::write(&query, [b">synthetic\n".as_slice(), &seq, b"\n"].concat()).unwrap();
    let mut writer = SketchDbWriter::new(dir.path().to_str().unwrap()).unwrap();
    let mut markers = Vec::new();
    for id in ["ref-b", "ref-a"] {
        let mut sketch = Sketch::new(1000, 125, 15, id.into(), false);
        sketch.contigs.push("synthetic".into());
        sketch.contig_lengths.push(seq.len() as u32);
        sketch.total_sequence_length = seq.len();
        skani::seeding::fmh_seeds(&seq, &params, 0, &mut sketch, true);
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
    let mut state = 7654321_u64;
    // Keep 20 kb shared; replace the rest with independently generated DNA.
    for base in &mut fasta[11 + 20_000..11 + 200_003] {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *base = b"ACGT"[(state & 3) as usize];
    }
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
    let mut entries: Vec<skani::sketch_db::IndexEntry> =
        bincode::deserialize(&fs::read(&index).unwrap()).unwrap();
    entries[0].offset = u64::MAX;
    fs::write(index, bincode::serialize(&entries).unwrap()).unwrap();
    assert!(SkaniDatabase::open(dir.path()).is_err());
    let (dir, query) = fixture();
    // Correct bounds, corrupt payload: fail the search rather than silently drop a hit.
    let payload = dir.path().join("sketches.db");
    let mut bytes = fs::read(&payload).unwrap();
    bytes[..8].fill(255);
    fs::write(payload, bytes).unwrap();
    let db = SkaniDatabase::open(dir.path()).unwrap();
    assert!(db
        .search(&query, SearchSettings::Primary, 1, SeedingMode::Auto)
        .is_err());
}
