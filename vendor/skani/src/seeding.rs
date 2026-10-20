use crate::embedded_skani::params::*;
use crate::embedded_skani::types::*;
use rust_lapper::{Interval, Lapper};

#[inline]
fn _position_min<T: Ord>(slice: &[T]) -> Option<usize> {
    slice
        .iter()
        .enumerate()
        .max_by(|(_, value0), (_, value1)| value1.cmp(value0))
        .map(|(idx, _)| idx)
}

pub fn get_nonoverlap_orf(sorted_orfs: Vec<Orf>) -> Vec<Orf> {
    type Iv = Interval<usize, u8>;
    let mut laps_f = Lapper::new(vec![]);
    let mut laps_rc = Lapper::new(vec![]);
    let mut ret = vec![];
    for orf in sorted_orfs {
        if orf.phase > 2 {
            if laps_rc.find(orf.start, orf.end).count() == 0 {
                let iv = Iv {
                    start: orf.start,
                    stop: orf.end,
                    val: orf.phase,
                };
                laps_rc.insert(iv);
            }
        } else if laps_f.find(orf.start, orf.end).count() == 0 {
            let iv = Iv {
                start: orf.start,
                stop: orf.end,
                val: orf.phase,
            };
            laps_f.insert(iv);
        }
    }
    for orf in laps_rc {
        ret.push(Orf {
            start: orf.start,
            end: orf.stop,
            phase: orf.val,
        });
    }
    for orf in laps_f {
        ret.push(Orf {
            start: orf.start,
            end: orf.stop,
            phase: orf.val,
        });
    }
    ret
}

pub fn get_orfs(string: &[u8], sketch_params: &SketchParams) -> Vec<Orf> {
    let mut orfs = vec![];
    let mut phase = 0;
    let mut rolling_3mer_f: MarkerBits = 0;
    let num_bits = std::mem::size_of::<MarkerBits>() * 8;
    let mut rolling_3mer_r: MarkerBits = 0;
    let reverse_shift_dist = num_bits - 2;
    let forward_shift_dist = num_bits - 6;

    let mut orf_pos_f = [0; 3];
    let mut orf_pos_r = [0; 3];

    for i in 0..string.len() {
        let nuc_f = BYTE_TO_SEQ[string[i] as usize];
        let nuc_r = 3 - nuc_f;
        rolling_3mer_f <<= 2;
        rolling_3mer_f |= nuc_f;
        rolling_3mer_r >>= 2;
        rolling_3mer_r |= nuc_r << reverse_shift_dist;

        if i >= 2 {
            let codon_f = sketch_params.acgt_to_aa_encoding[(rolling_3mer_f & 63) as usize];
            if codon_f == STOP_CODON {
                if orf_pos_f[phase] != 0 && (i - 2) - orf_pos_f[phase] > ORF_SIZE {
                    orfs.push(Orf {
                        start: orf_pos_f[phase],
                        end: (i - 2),
                        phase: phase as u8,
                    });
                }
                orf_pos_f[phase] = i - 2;
            }
            let codon_r =
                sketch_params.acgt_to_aa_encoding[(rolling_3mer_r >> forward_shift_dist) as usize];

            if codon_r == STOP_CODON {
                if orf_pos_f[phase] != 0 && (i - 2) - orf_pos_r[phase] > 45 {
                    orfs.push(Orf {
                        start: orf_pos_r[phase],
                        end: (i - 2),
                        phase: phase as u8 + 3,
                    });
                }
                orf_pos_r[phase] = i - 2;
            }
        }
        phase += 1;
        if phase == 3 {
            phase = 0;
        }
    }

    orfs.sort_by(|x, y| (y.end - y.start).cmp(&(x.end - x.start)));
    orfs
    //    let non_ol_orfs = get_nonoverlap_orf(orfs);
    //    dbg!(non_ol_orfs.len());
    //    return non_ol_orfs;
}

pub fn fmh_seeds_aa_with_orf(
    string: &[u8],
    sketch_params: &SketchParams,
    contig_index: ContigIndex,
    new_sketch: &mut Sketch,
    orfs: Vec<Orf>,
    seed: bool,
) {
    let marker_k = K_MARKER_AA;
    if seed && new_sketch.kmer_seeds_k.is_none() {
        new_sketch.kmer_seeds_k = Some(KmerSeeds::default());
    }
    let _kmer_seeds_k = &mut new_sketch.kmer_seeds_k;
    let k = sketch_params.k;
    let c = sketch_params.c;
    let kmer_to_aa_table = &sketch_params.acgt_to_aa_encoding;
    if k > marker_k {
        panic!("Value of k > {} for AA; not allowed.", marker_k);
    }
    let marker_k = K_MARKER_AA;
    if string.len() < 2 * marker_k {
        return;
    }

    let num_bits = std::mem::size_of::<MarkerBits>() * 8;

    let marker_reverse_shift_dist_aa = 5 * (marker_k - 1);
    let marker_max_mask_aa = MarkerBits::MAX >> (num_bits - 5 * marker_k);

    let reverse_shift_dist = 3 * 2 * marker_k - 2;
    let reverse_shift_dist_aa = 5 * (k - 1);
    let forward_shift_rc = marker_k * 3 * 2 - 6;
    let max_mask = MarkerBits::MAX >> (num_bits - 3 * 2 * marker_k);
    let max_mask_aa = MarkerBits::MAX >> (num_bits - 5 * k);
    let max_rev_mask = !(3 << (3 * 2 * marker_k - 2));
    //    let max_rev_mask_aa = !(0 | (31 << (5 * (marker_k - 1))));
    let three_mer_mask = 63;
    let threshold = u64::MAX / (c as u64);
    let marker_threshold = u64::MAX / sketch_params.marker_c as u64;
    for orf in orfs {
        //        dbg!(&orf);
        let phase = orf.phase;
        let rc = phase > 2;
        let start = orf.start;
        let end = orf.end + 3;
        let range = start..end;
        let mut rolling_kmer: MarkerBits = 0;
        let mut rolling_aa_kmer = 0;
        let mut marker_rolling_aa_kmer = 0;

        let mut j = 0;
        for i in range {
            let nuc_f = BYTE_TO_SEQ[string[i] as usize];
            if !rc {
                rolling_kmer <<= 2;
                rolling_kmer |= nuc_f;
                rolling_kmer &= max_mask;
            } else {
                let nuc_r = 3 - nuc_f;
                rolling_kmer >>= 2;
                rolling_kmer &= max_rev_mask;
                rolling_kmer |= nuc_r << reverse_shift_dist;
                rolling_kmer &= max_mask;
            }
            if j >= 2 && (j - 2) % 3 == 0 {
                if !rc {
                    let temp_aa = kmer_to_aa_table[(rolling_kmer & three_mer_mask) as usize];
                    marker_rolling_aa_kmer <<= 5;
                    marker_rolling_aa_kmer |= temp_aa;
                    marker_rolling_aa_kmer &= marker_max_mask_aa;

                    rolling_aa_kmer <<= 5;
                    rolling_aa_kmer |= temp_aa;
                    rolling_aa_kmer &= max_mask_aa;
                } else {
                    let temp_aa = kmer_to_aa_table[(rolling_kmer >> forward_shift_rc) as usize];

                    marker_rolling_aa_kmer >>= 5;
                    marker_rolling_aa_kmer |= temp_aa << marker_reverse_shift_dist_aa;

                    rolling_aa_kmer >>= 5;
                    rolling_aa_kmer |= temp_aa << reverse_shift_dist_aa;
                }

                if j >= marker_k * 3 - 1 {
                    let hash = mm_hash64(rolling_aa_kmer);
                    if hash < threshold {
                        if seed {
                            //dbg!(rolling_aa_kmer, rolling_aa_kmer as SeedBits);
                            new_sketch.add_seed_position(
                                rolling_aa_kmer as SeedBits,
                                SeedPosition::new(i as GnPosition, contig_index, !rc),
                            );
                        }
                        if hash < marker_threshold && j >= marker_k * 3 - 1 {
                            new_sketch.marker_seeds.insert(marker_rolling_aa_kmer);
                        }
                    }
                }
            }
            j += 1;
        }
    }

    //kmer_seeds_k].shrink_to_fit();
}

/// Portable reproduction of the v0.3.2 AVX2 DNA seeding contract.
/// Four overlapping lanes, truncated tail, uppercase-N resume after 21 bases,
/// and lane-interleaved insertion are intentional frozen compatibility semantics.
/// Lowercase n is encoded through BYTE_TO_SEQ without the uppercase-N pause.
pub fn fmh_seeds(
    string: &[u8],
    sketch_params: &SketchParams,
    contig_index: ContigIndex,
    new_sketch: &mut Sketch,
    seed: bool,
) {
    if seed && new_sketch.kmer_seeds_k.is_none() {
        new_sketch.kmer_seeds_k = Some(KmerSeeds::default());
    }
    let marker_k = K_MARKER_DNA;
    let k = sketch_params.k;
    let c = sketch_params.c;
    let marker_c = sketch_params.marker_c;
    if k > marker_k {
        panic!("Value of k > {} for DNA; not allowed.", marker_k);
    }
    if string.len() < 2 * marker_k {
        return;
    }
    let len = (string.len() - marker_k + 1) / 4;
    let strs = [
        &string[0..len + marker_k - 1],
        &string[len..2 * len + marker_k - 1],
        &string[2 * len..3 * len + marker_k - 1],
        &string[3 * len..4 * len + marker_k - 1],
    ];
    let mut f = [0u64; 4];
    let mut r = [0u64; 4];
    for i in 0..marker_k - 1 {
        for l in 0..4 {
            let nf = BYTE_TO_SEQ[strs[l][i] as usize];
            f[l] = (f[l] << 2) | nf;
            r[l] = (r[l] >> 2) | ((3 - nf) << 40);
        }
    }
    let seed_mask = MarkerBits::MAX >> (std::mem::size_of::<MarkerBits>() * 8 - 2 * k);
    let marker_mask = MarkerBits::MAX >> (std::mem::size_of::<MarkerBits>() * 8 - 2 * marker_k);
    let rev_marker_mask: u64 = !(3 << (2 * marker_k - 2));
    let threshold_unsigned = u64::MAX / c as u64;
    let threshold_marker_unsigned = u64::MAX / marker_c as u64;
    let mut resume_inds = [0usize; 4];
    for i in marker_k - 1..(len + marker_k - 1) {
        let mut canonical = [false; 4];
        let mut canonical_seeds = [0u64; 4];
        let mut canonical_marker = [false; 4];
        let mut hashes = [0u64; 4];
        for l in 0..4 {
            let b = strs[l][i] as usize;
            if b == ASCII_N {
                resume_inds[l] = i + marker_k;
            }
            let nf = BYTE_TO_SEQ[b];
            f[l] = ((f[l] << 2) | nf) & marker_mask;
            r[l] = ((r[l] >> 2) & rev_marker_mask) | ((3 - nf) << 40);
            let fs = f[l] & seed_mask;
            let rs = r[l] & seed_mask;
            canonical[l] = (rs as i64) > (fs as i64);
            canonical_seeds[l] = if canonical[l] { fs } else { rs };
            canonical_marker[l] = (r[l] as i64) > (f[l] as i64);
            hashes[l] = mm_hash64(canonical_seeds[l]);
        }
        for l in 0..4 {
            if hashes[l] < threshold_unsigned && resume_inds[l] <= i {
                let pos = (i + len * l) as GnPosition;
                new_sketch.add_seed_position(
                    canonical_seeds[l] as i64 as SeedBits,
                    SeedPosition::new(pos, contig_index, canonical[l]),
                );
                let canonical_kmer_marker = if canonical_marker[l] { f[l] } else { r[l] };
                if hashes[l] < threshold_marker_unsigned {
                    new_sketch.marker_seeds.insert(canonical_kmer_marker);
                }
            }
        }
    }
}

pub fn get_repetitive_kmers(kmer_seeds: &Option<KmerSeeds>, sketch: &Sketch, c: usize) -> usize {
    if kmer_seeds.is_none() {
        usize::MAX
    } else {
        let kmer_seeds = kmer_seeds.as_ref().unwrap();
        let mut count_vec = vec![];
        for (seed, _tagged_index) in kmer_seeds {
            // Use the sketch's get_seed_positions to get the actual count
            let positions_iter = sketch.get_seed_positions(*seed);
            count_vec.push(positions_iter.len());
        }
        count_vec.sort();
        let mut max_repet_cutoff = count_vec[count_vec.len() - count_vec.len() / 5000 - 1];
        let default_repet_cutoff = BP_CHAIN_BAND / c;
        if max_repet_cutoff < 20 || max_repet_cutoff > default_repet_cutoff {
            max_repet_cutoff = default_repet_cutoff;
        }
        max_repet_cutoff
    }
}
