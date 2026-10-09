//! Direct tests of the pinned seeding contract, using native AVX2 as an independent oracle.
use skani::params::SketchParams;
use skani::types::Sketch;

fn sequence(len: usize) -> Vec<u8> {
    let mut state = 123456789_u64;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            b"ACGT"[(state & 3) as usize]
        })
        .collect()
}

fn digest(sketch: &Sketch) -> u64 {
    let mut values = Vec::new();
    let mut keys: Vec<_> = sketch
        .kmer_seeds_k
        .as_ref()
        .unwrap()
        .keys()
        .copied()
        .collect();
    keys.sort();
    for key in keys {
        values.push(u64::from(key));
        for pos in sketch.get_seed_positions(key).iter() {
            values.push(pos.pack_to_u64());
        }
        values.push(u64::MAX);
    }
    let mut markers: Vec<_> = sketch.marker_seeds.iter().copied().collect();
    markers.sort();
    values.extend(markers);
    values
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .fold(0xcbf29ce484222325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        })
}

#[test]
fn portable_seeds_match_frozen_native_vectors_on_every_architecture() {
    // Native AVX2 oracle, source revision and derivation in skani-fixtures.md.
    for (len, expected) in [
        (500, 0x2b44f99ab893d981),
        (501, 0x2b44f99ab893d981),
        (502, 0x2b44f99ab893d981),
        (503, 0x2b44f99ab893d981),
    ] {
        let mut seq = sequence(len);
        seq[119] = b'N';
        seq[120] = b'n';
        seq[240..262].fill(b'N');
        let params = SketchParams::new(1, 1, 15, false, false);
        let mut portable = Sketch::new(1, 1, 15, String::new(), false);
        skani::seeding::fmh_seeds(&seq, &params, 7, &mut portable, true);
        assert_eq!(digest(&portable), expected, "length {len}");
    }
}

#[test]
fn short_contigs_are_safe_and_marker_only_seeding_does_not_add_positions() {
    let params = SketchParams::new(1, 1, 15, false, false);
    for len in 0..42 {
        let mut sketch = Sketch::new(1, 1, 15, String::new(), false);
        skani::seeding::fmh_seeds(&sequence(len), &params, 0, &mut sketch, true);
        assert!(sketch.kmer_seeds_k.as_ref().unwrap().is_empty());
        assert!(sketch.marker_seeds.is_empty());
        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            unsafe {
                skani::avx2_seeding::avx2_fmh_seeds(&sequence(len), &params, 0, &mut sketch, true)
            };
            assert!(sketch.kmer_seeds_k.as_ref().unwrap().is_empty());
        }
    }
    let mut sketch = Sketch::new(1, 1, 15, String::new(), false);
    skani::seeding::fmh_seeds(&sequence(503), &params, 0, &mut sketch, false);
    assert!(sketch.kmer_seeds_k.is_none());
    assert!(!sketch.marker_seeds.is_empty());
}

#[cfg(target_arch = "x86_64")]
fn compare(seq: &[u8]) {
    if !is_x86_feature_detected!("avx2") {
        return;
    }
    for (c, marker_c) in [(1, 1), (30, 200), (125, 1000)] {
        let params = SketchParams::new(marker_c, c, 15, false, false);
        let mut native = Sketch::new(marker_c, c, 15, String::new(), false);
        let mut portable = native.clone();
        // SAFETY: runtime detection above establishes the AVX2 requirement.
        unsafe { skani::avx2_seeding::avx2_fmh_seeds(seq, &params, 7, &mut native, true) };
        skani::seeding::fmh_seeds(seq, &params, 7, &mut portable, true);
        assert!(native == portable, "length {}, c {c}", seq.len());
        // A second contig verifies packed contig indices and repeated-seed order.
        unsafe { skani::avx2_seeding::avx2_fmh_seeds(seq, &params, 8, &mut native, true) };
        skani::seeding::fmh_seeds(seq, &params, 8, &mut portable, true);
        assert!(
            native == portable,
            "second contig, length {}, c {c}",
            seq.len()
        );
    }
}

#[cfg(target_arch = "x86_64")]
#[test]
fn all_four_contig_length_remainders_match_native_avx2() {
    for len in (42..58).chain(500..516).chain(1000..1008) {
        compare(&sequence(len));
    }
}

#[cfg(target_arch = "x86_64")]
#[test]
fn uppercase_and_lowercase_n_at_lane_boundaries_match_native_avx2() {
    for len in 500..504 {
        let lane_len = (len - 20) / 4;
        for boundary in [
            0,
            20,
            lane_len,
            lane_len + 20,
            2 * lane_len,
            2 * lane_len + 20,
            3 * lane_len,
            3 * lane_len + 20,
            len - 1,
        ] {
            for offset in -1_isize..=1 {
                let start = boundary as isize + offset;
                if start < 0 || start >= len as isize {
                    continue;
                }
                for base in [b'N', b'n'] {
                    for count in [1, 20, 21, 22] {
                        let mut seq = sequence(len);
                        seq[start as usize..(start as usize + count).min(len)].fill(base);
                        compare(&seq);
                    }
                }
            }
        }
    }
}
