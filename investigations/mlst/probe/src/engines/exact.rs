//! Route 2: exact-allele hash lookup on both strands, then k-mer seeding and a pairwise-alignment crate for
//! loci that have no exact allele. The alignment only nominates diagonals; HSPs come from the shared finisher.
use super::ungapped::{kmers, unpack, FxMap, SeedIndex};
use super::{finish, Diag, Oriented};
use crate::fasta::Contig;
use crate::hsp::Hsp;
use crate::package::{Package, LOCI};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Instant;

pub const SEED_K: usize = 20;
pub const TOP_ALLELES: usize = 32;
const CLUSTER_GAP: i64 = 64;
const WINDOW_SLOP: i64 = 24;

#[derive(Clone, Copy, PartialEq)]
pub enum Pairwise {
    None,
    #[cfg(feature = "engine-exact-bio")]
    Bio,
    #[cfg(feature = "engine-exact-blockaligner")]
    BlockAligner,
}

#[derive(Default, serde::Serialize)]
pub struct Stats {
    pub exact_seconds: f64,
    pub seed_index_seconds: f64,
    pub seeding_seconds: f64,
    pub alignment_seconds: f64,
    pub loci_without_exact: usize,
    pub alignments: usize,
}

const B: u64 = 0x9E3779B97F4A7C15 | 1;

fn hash_seq(s: &[u8]) -> u64 {
    s.iter().fold(0u64, |h, &b| h.wrapping_mul(B).wrapping_add(b as u64 + 1))
}

/// Exact two-strand allele lookup by length class with a rolling hash, verified byte-for-byte.
fn exact_hits(pkg: &Package, ori: &Oriented, contigs: &[Contig]) -> Vec<Hsp> {
    let mut by_len: BTreeMap<usize, FxMap<u64, Vec<(u32, bool)>>> = BTreeMap::new();
    for (i, a) in pkg.alleles.iter().enumerate() {
        let e = by_len.entry(a.seq.len()).or_default();
        e.entry(hash_seq(&a.seq)).or_default().push((i as u32, false));
        e.entry(hash_seq(&ori.rc[i])).or_default().push((i as u32, true));
    }
    let mut out = Vec::new();
    for (&len, table) in &by_len {
        let top = (0..len).fold(1u64, |p, _| p.wrapping_mul(B)); // B^len
        for (ci, c) in contigs.iter().enumerate() {
            if c.seq.len() < len { continue; }
            let s = &c.seq;
            let (mut h, mut ns) = (0u64, 0usize);
            for i in 0..s.len() {
                h = h.wrapping_mul(B).wrapping_add(s[i] as u64 + 1);
                ns += (s[i] == b'N') as usize;
                if i >= len {
                    h = h.wrapping_sub((s[i - len] as u64 + 1).wrapping_mul(top));
                    ns -= (s[i - len] == b'N') as usize;
                }
                if i + 1 >= len && ns == 0 {
                    if let Some(cands) = table.get(&h) {
                        let start = i + 1 - len;
                        for &(a, minus) in cands {
                            if ori.seq(pkg, a as usize, minus) == &s[start..=i] {
                                out.push(Hsp { contig: ci, allele: a as usize, start, end: i + 1, minus, nident: len as u32, alen: len as u32, score: len as i32 });
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

#[cfg(feature = "engine-exact-bio")]
fn align_bio(aligner: &mut bio::alignment::pairwise::Aligner<fn(u8, u8) -> i32>, x: &[u8], y: &[u8], y_off: i64, out: &mut Vec<(i64, i64)>) {
    use bio::alignment::AlignmentOperation as Op;
    let al = aligner.local(x, y);
    let (mut xp, mut yp) = (al.xstart as i64, al.ystart as i64);
    let mut in_block = false;
    for op in &al.operations {
        match op {
            Op::Match | Op::Subst => {
                if !in_block { out.push((y_off + yp - xp, 0)); in_block = true; }
                xp += 1; yp += 1;
            }
            Op::Ins => { xp += 1; in_block = false; }
            Op::Del => { yp += 1; in_block = false; }
            _ => {}
        }
    }
}

#[cfg(feature = "engine-exact-bio")]
fn bio_score(a: u8, b: u8) -> i32 { if a == b && a != b'N' { 1 } else { -2 } }

pub fn run(pkg: &Package, ori: &Oriented, contigs: &[Contig], pairwise: Pairwise) -> (Vec<Hsp>, Stats) {
    let mut stats = Stats::default();
    let t = Instant::now();
    let mut hsps = exact_hits(pkg, ori, contigs);
    stats.exact_seconds = t.elapsed().as_secs_f64();
    let mut has_exact = [false; 7];
    for h in &hsps { has_exact[pkg.alleles[h.allele].locus] = true; }
    let missing: Vec<usize> = (0..LOCI.len()).filter(|&l| !has_exact[l]).collect();
    stats.loci_without_exact = missing.len();
    if missing.is_empty() || pairwise == Pairwise::None {
        return (hsps, stats);
    }
    let t = Instant::now();
    let index = SeedIndex::build(pkg, ori, SEED_K);
    stats.seed_index_seconds = t.elapsed().as_secs_f64();
    let t = Instant::now();
    // (locus, contig, minus) -> list of (diag, allele)
    let mut seeds: HashMap<(usize, usize, bool), Vec<(i64, u32, usize)>> = HashMap::new();
    for (ci, c) in contigs.iter().enumerate() {
        kmers(&c.seq, SEED_K, |p, km| {
            for &post in index.get(km) {
                let (a, minus, ap) = unpack(post);
                let locus = pkg.alleles[a].locus;
                if !has_exact[locus] {
                    seeds.entry((locus, ci, minus)).or_default().push((p as i64 - ap as i64, a as u32, p));
                }
            }
        });
    }
    stats.seeding_seconds = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let mut diags: HashSet<Diag> = HashSet::new();
    #[cfg(feature = "engine-exact-bio")]
    let mut bio = bio::alignment::pairwise::Aligner::new(-5, -2, bio_score as fn(u8, u8) -> i32);
    let mut keys: Vec<_> = seeds.keys().copied().collect();
    keys.sort();
    for key in keys {
        let (_locus, ci, minus) = key;
        let list = seeds.get_mut(&key).unwrap();
        list.sort();
        let mut start = 0;
        while start < list.len() {
            let mut end = start + 1;
            while end < list.len() && list[end].0 - list[end - 1].0 <= CLUSTER_GAP { end += 1; }
            let cluster = &list[start..end];
            start = end;
            let mut counts: HashMap<u32, u32> = HashMap::new();
            for &(_, a, _) in cluster { *counts.entry(a).or_default() += 1; }
            let mut ranked: Vec<(u32, u32)> = counts.into_iter().collect();
            ranked.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));
            ranked.truncate(TOP_ALLELES);
            let (dmin, dmax) = (cluster.first().unwrap().0, cluster.last().unwrap().0);
            let lmax = ranked.iter().map(|&(a, _)| pkg.alleles[a as usize].seq.len() as i64).max().unwrap();
            let clen = contigs[ci].seq.len() as i64;
            let w0 = (dmin - WINDOW_SLOP).max(0);
            let w1 = (dmax + lmax + WINDOW_SLOP).min(clen);
            if w1 <= w0 { continue; }
            #[cfg(feature = "engine-exact-bio")]
            let window = &contigs[ci].seq[w0 as usize..w1 as usize];
            for &(a, _) in &ranked {
                let x = ori.seq(pkg, a as usize, minus);
                let mut found: Vec<(i64, i64)> = Vec::new();
                match pairwise {
                    #[cfg(feature = "engine-exact-bio")]
                    Pairwise::Bio => align_bio(&mut bio, x, window, w0, &mut found),
                    #[cfg(feature = "engine-exact-blockaligner")]
                    Pairwise::BlockAligner => {
                        let anchor = cluster.iter().filter(|s| s.1 == a).min_by_key(|s| s.2).unwrap();
                        block_aligner_diags(x, &contigs[ci].seq, anchor.0, anchor.2, &mut found)
                    }
                    Pairwise::None => {}
                }
                stats.alignments += 1;
                for (d, _) in found { diags.insert((a, minus, ci as u32, d)); }
            }
        }
    }
    stats.alignment_seconds = t.elapsed().as_secs_f64();
    hsps.extend(finish(pkg, ori, contigs, &diags));
    (hsps, stats)
}

/// Anchored X-drop extension (block-aligner has global and X-drop modes, not local) from one seed,
/// to the right and, on reversed sequences, to the left. `ap` = allele position of the anchor seed.
#[cfg(feature = "engine-exact-blockaligner")]
fn block_aligner_diags(x: &[u8], contig: &[u8], d0: i64, cp: usize, out: &mut Vec<(i64, i64)>) {
    use block_aligner::{cigar::*, scan_block::*, scores::*};
    let ap = (cp as i64 - d0) as usize;
    let gaps = Gaps { open: -5, extend: -2 };
    let matrix = NucMatrix::new_simple(1, -2);
    let (min_b, max_b) = (32usize, 64usize);
    for reverse in [false, true] {
        let (q, r): (Vec<u8>, Vec<u8>) = if !reverse {
            (x[ap..].to_vec(), contig[cp..(cp + (x.len() - ap) + 32).min(contig.len())].to_vec())
        } else {
            (x[..ap].iter().rev().copied().collect(), contig[cp.saturating_sub(ap + 32)..cp].iter().rev().copied().collect())
        };
        if q.is_empty() || r.is_empty() { continue; }
        let qp = PaddedBytes::from_bytes::<NucMatrix>(&q, max_b);
        let rp = PaddedBytes::from_bytes::<NucMatrix>(&r, max_b);
        let mut a = Block::<true, true>::new(q.len(), r.len(), max_b);
        a.align(&qp, &rp, &matrix, gaps, min_b..=max_b, 20);
        let res = a.res();
        let mut cigar = Cigar::new(res.query_idx, res.reference_idx);
        a.trace().cigar(res.query_idx, res.reference_idx, &mut cigar);
        let (mut i, mut j) = (0i64, 0i64);
        let mut in_block = false;
        let mut ops = cigar.to_vec();
        ops.retain(|o| o.len > 0 && o.op != Operation::Sentinel);
        for o in ops {
            let n = o.len as i64;
            match o.op {
                Operation::M | Operation::Eq | Operation::X => {
                    if !in_block { out.push((if reverse { d0 - (j - i) } else { d0 + (j - i) }, 0)); in_block = true; }
                    i += n; j += n;
                }
                Operation::I => { i += n; in_block = false; }
                Operation::D => { j += n; in_block = false; }
                Operation::Sentinel => {}
            }
        }
    }
}
