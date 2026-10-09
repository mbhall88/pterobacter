mod common;
#[test]
fn query_file_label_preserves_native_orientation_tie_values() {
    use skani::{params::SketchParams, sketch_db::SketchDbWriter, types::Sketch};
    use std::fs;
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
    skani::seeding::fmh_seeds(&seq, &params, 0, &mut query, true);
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
    skani::seeding::fmh_seeds(&reference_seq, &params, 0, &mut reference, true);
    assert_eq!(reference.marker_seeds.len(), query.marker_seeds.len());
    let mut writer = SketchDbWriter::new(root.to_str().unwrap()).unwrap();
    writer.add_sketch(&params, &reference).unwrap();
    writer.finalize(root.to_str().unwrap()).unwrap();
    fs::write(
        root.join("markers.bin"),
        bincode::serialize(&(params, vec![Sketch::get_markers_only(&reference)])).unwrap(),
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
    assert_eq!(report.rows[0].ani.to_string(), "91.97");
    assert_eq!(report.rows[0].af_query.to_string(), "95.80");
    assert_eq!(report.rows[0].af_reference.to_string(), "95.80");
}
