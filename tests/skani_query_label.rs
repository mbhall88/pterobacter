mod common;
#[test]
fn orientation_ties_ignore_query_paths_and_preserve_labels() {
    use std::fs;
    use terobacter::embedded_skani::{
        params::SketchParams, sketch_db::SketchDbWriter, types::Sketch,
    };
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let query_path = root.join("query.fa");
    // A proper extension sorts after its prefix, regardless of TMPDIR.
    let reference_label = format!("{}.reference", query_path.display());
    let params = SketchParams::new(1000, 125, 15, false, false);
    let seq = common::dna(200_003, 42);
    let mut query = Sketch::new(1000, 125, 15, "query".into(), false);
    query.contigs.push("synthetic".into());
    query.contig_lengths.push(seq.len() as u32);
    query.total_sequence_length = seq.len();
    terobacter::embedded_skani::seeding::fmh_seeds(&seq, &params, 0, &mut query, true);
    let mut reference_seq = seq.clone();
    let mut state = 1_u64;
    for base in &mut reference_seq {
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
    let mut reference = Sketch::new(1000, 125, 15, reference_label, false);
    reference.contigs.push("synthetic".into());
    reference.contig_lengths.push(seq.len() as u32);
    reference.total_sequence_length = seq.len();
    terobacter::embedded_skani::seeding::fmh_seeds(
        &reference_seq,
        &params,
        0,
        &mut reference,
        true,
    );
    assert_eq!(reference.marker_seeds.len(), query.marker_seeds.len());
    let mut writer = SketchDbWriter::new(root.to_str().unwrap()).unwrap();
    writer.add_sketch(&params, &reference).unwrap();
    writer.finalize(root.to_str().unwrap()).unwrap();
    fs::write(
        root.join("markers.bin"),
        bincode::serialize(&(&params, vec![Sketch::get_markers_only(&reference)])).unwrap(),
    )
    .unwrap();
    fs::write(
        root.join("query.fa"),
        [b">synthetic\n".as_slice(), &seq, b"\n"].concat(),
    )
    .unwrap();
    let db = terobacter::skani::SkaniDatabase::open(root).unwrap();
    let report = db
        .search(
            &query_path,
            terobacter::skani::SearchSettings::Primary,
            1,
            terobacter::skani::SeedingMode::Auto,
        )
        .unwrap();
    assert_eq!(report.rows.len(), 1);
    assert_eq!(
        report.engine_profile,
        "skani-0.3.2-avx2-content-order-dna-v2"
    );
    let bytes = fs::read(&query_path).unwrap();
    for settings in [
        terobacter::skani::SearchSettings::Primary,
        terobacter::skani::SearchSettings::Genomovar,
    ] {
        for threads in [1, 2] {
            for mode in [
                terobacter::skani::SeedingMode::Auto,
                terobacter::skani::SeedingMode::Portable,
            ] {
                for label in [
                    "/a/query.fa",
                    "/f/query.fa",
                    "/z/query.fa",
                    "different sample name",
                ] {
                    let other = db
                        .search_bytes(label, &bytes, settings, threads, mode)
                        .unwrap();
                    assert_eq!(report.rows, other.rows, "path={label}");
                    assert_eq!(
                        report.not_returned_reference_files,
                        other.not_returned_reference_files
                    );
                }
            }
        }
    }
    // The fixture is a known upstream orientation-sensitive pair. Pin the corrected profile's
    // estimate separately; the historical 91.97 orientation remains recorded in the research baseline.
    assert_eq!(report.rows[0].ani.to_string(), "91.97");
    assert_eq!(report.rows[0].af_reference.to_string(), "95.80");
    assert_eq!(report.rows[0].af_query.to_string(), "95.80");

    // Move the reference label to the other side of every query label. The package
    // authenticates labels separately; numerical measurement must not depend on them.
    let renamed = tempfile::tempdir().unwrap();
    reference.file_name = "/z/renamed-reference.fa".into();
    let mut writer = SketchDbWriter::new(renamed.path().to_str().unwrap()).unwrap();
    writer.add_sketch(&params, &reference).unwrap();
    writer.finalize(renamed.path().to_str().unwrap()).unwrap();
    fs::write(
        renamed.path().join("markers.bin"),
        bincode::serialize(&(&params, vec![Sketch::get_markers_only(&reference)])).unwrap(),
    )
    .unwrap();
    let renamed_db = terobacter::skani::SkaniDatabase::open(renamed.path()).unwrap();
    let mut inputs = vec![bytes.clone()];
    for extension in ["gz", "bz2", "xz", "zst"] {
        inputs.push(
            fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                    "tests/fixtures/skani-orientation/query.fa.{extension}"
                )),
            )
            .unwrap(),
        );
    }
    for input in &inputs {
        for label in ["/a/renamed-query.fa", "/zz/query.fa.gz"] {
            let mut other = renamed_db
                .search_bytes(
                    label,
                    input,
                    terobacter::skani::SearchSettings::Primary,
                    1,
                    terobacter::skani::SeedingMode::Portable,
                )
                .unwrap();
            assert_eq!(other.total_input_bases, report.total_input_bases);
            assert_eq!(other.sketched_bases, report.sketched_bases);
            assert_eq!(other.rows.len(), 1);
            assert_eq!(other.rows[0].reference_file, "/z/renamed-reference.fa");
            other.rows[0]
                .reference_file
                .clone_from(&report.rows[0].reference_file);
            assert_eq!(other.rows, report.rows);
        }
    }
    // Reverse actual measurement roles, not just the comparator.
    let reciprocal = tempfile::tempdir().unwrap();
    query.file_name = "/another/reference".into();
    let mut writer = SketchDbWriter::new(reciprocal.path().to_str().unwrap()).unwrap();
    writer.add_sketch(&params, &query).unwrap();
    writer
        .finalize(reciprocal.path().to_str().unwrap())
        .unwrap();
    fs::write(
        reciprocal.path().join("markers.bin"),
        bincode::serialize(&(&params, vec![Sketch::get_markers_only(&query)])).unwrap(),
    )
    .unwrap();
    let reciprocal_db = terobacter::skani::SkaniDatabase::open(reciprocal.path()).unwrap();
    let reciprocal_bytes = [b">synthetic\n".as_slice(), &reference_seq, b"\n"].concat();
    let reversed = reciprocal_db
        .search_bytes(
            "/opposite/query",
            &reciprocal_bytes,
            terobacter::skani::SearchSettings::Primary,
            2,
            terobacter::skani::SeedingMode::Portable,
        )
        .unwrap();
    assert_eq!(reversed.rows.len(), 1);
    assert_eq!(reversed.rows[0].ani, report.rows[0].ani);
    assert_eq!(reversed.rows[0].af_reference, report.rows[0].af_query);
    assert_eq!(reversed.rows[0].af_query, report.rows[0].af_reference);
}
