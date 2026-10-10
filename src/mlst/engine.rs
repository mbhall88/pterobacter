//! Route D from the MLST feasibility study: exact 32-mer seeds between the allele set and the
//! assembly, each finished by an ungapped X-drop extension with megablast +1/−2 scoring. This
//! reproduces the HSPs of `blastn -ungapped -word_size 32` on the development fixtures; it is a
//! separate engine, not BLAST.
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

pub(crate) const WORD: usize = 32;
pub(crate) const REWARD: i32 = 1;
pub(crate) const PENALTY: i32 = -2;
/// Raw ungapped X-drop. A sweep over the development fixtures brackets the comparator to 12–14;
/// the midpoint is used. The value was fitted in-sample.
pub(crate) const XDROP: i32 = 13;
/// Allele positions are packed into 10 bits and allele indices into 21.
pub(crate) const MAX_ALLELE_LEN: usize = 1 << 10;
pub(crate) const MAX_ALLELES: usize = 1 << 21;

/// One ungapped high-scoring segment pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Hsp {
    pub contig: usize,
    /// Allele index in database order.
    pub allele: usize,
    /// Half-open forward-strand contig interval.
    pub start: usize,
    pub end: usize,
    pub minus: bool,
    pub nident: u32,
    pub alen: u32,
    pub score: i32,
}

pub(crate) fn revcomp(s: &[u8]) -> Vec<u8> {
    s.iter()
        .rev()
        .map(|&b| match b {
            b'A' => b'T',
            b'C' => b'G',
            b'G' => b'C',
            b'T' => b'A',
            _ => b'N',
        })
        .collect()
}

#[inline]
fn is_match(a: u8, b: u8) -> bool {
    a == b && a != b'N'
}

/// Finish one diagonal. `allele` is oriented to match the forward contig and `diag` is
/// contig position − allele position. Each maximal exact run of at least `WORD` bases not
/// already inside an HSP seeds an extension in both directions that keeps the first maximum.
fn extend_diagonal(
    allele: &[u8],
    contig: &[u8],
    diag: isize,
    contig_idx: usize,
    allele_idx: usize,
    minus: bool,
    out: &mut Vec<Hsp>,
) {
    let l = allele.len() as isize;
    let q_lo = (-diag).max(0);
    let q_hi = l.min(contig.len() as isize - diag);
    if q_hi - q_lo < WORD as isize {
        return;
    }
    let (q_lo, q_hi) = (q_lo as usize, q_hi as usize);
    let at = |q: usize| contig[(q as isize + diag) as usize];
    let mut covered_to = q_lo;
    let mut q = q_lo;
    while q < q_hi {
        if !is_match(allele[q], at(q)) {
            q += 1;
            continue;
        }
        let run_start = q;
        while q < q_hi && is_match(allele[q], at(q)) {
            q += 1;
        }
        let run_end = q;
        if run_end - run_start < WORD || run_start < covered_to {
            continue;
        }
        let (mut best, mut cur, mut right) = (0, 0, run_end);
        let mut i = run_end;
        while i < q_hi {
            cur += if is_match(allele[i], at(i)) {
                REWARD
            } else {
                PENALTY
            };
            i += 1;
            if cur > best {
                best = cur;
                right = i;
            } else if best - cur >= XDROP {
                break;
            }
        }
        let (mut lbest, mut lcur, mut left) = (0, 0, run_start);
        let mut i = run_start;
        while i > q_lo {
            i -= 1;
            lcur += if is_match(allele[i], at(i)) {
                REWARD
            } else {
                PENALTY
            };
            if lcur > lbest {
                lbest = lcur;
                left = i;
            } else if lbest - lcur >= XDROP {
                break;
            }
        }
        let alen = right - left;
        let nident = (left..right)
            .filter(|&i| is_match(allele[i], at(i)))
            .count() as u32;
        let score = nident as i32 * REWARD + (alen as u32 - nident) as i32 * PENALTY;
        out.push(Hsp {
            contig: contig_idx,
            allele: allele_idx,
            start: (left as isize + diag) as usize,
            end: (right as isize + diag) as usize,
            minus,
            nident,
            alen: alen as u32,
            score,
        });
        covered_to = right;
        q = q.max(right);
    }
}

/// Smallest raw score passing `-evalue` for a contig of `m` bases against `n` database bases,
/// with ungapped +1/−2 statistics and the usual edge-effect correction.
pub(crate) fn min_score_for_evalue(m: usize, n: usize, evalue: f64) -> i32 {
    let (lambda, k, h) = (1.28f64, 0.46f64, 0.85f64);
    let (m, n) = (m as f64, n as f64);
    let l = ((k * m * n).ln() / h).max(0.0);
    let (me, ne) = ((m - l).max(1.0), (n - l).max(1.0));
    ((k * me * ne / evalue).ln() / lambda).ceil() as i32
}

#[derive(Default)]
struct Fx(u64);
impl Hasher for Fx {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(5) ^ b as u64).wrapping_mul(0x517cc1b727220a95);
        }
    }
    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x517cc1b727220a95);
    }
}
type FxMap<K, V> = HashMap<K, V, BuildHasherDefault<Fx>>;

/// Calls `f(window_start, kmer)` for every window of `WORD` unambiguous bases.
fn kmers(seq: &[u8], mut f: impl FnMut(usize, u64)) {
    let (mut cur, mut valid) = (0u64, 0usize);
    for (i, &b) in seq.iter().enumerate() {
        let code = match b {
            b'A' => 0,
            b'C' => 1,
            b'G' => 2,
            b'T' => 3,
            _ => {
                valid = 0;
                cur = 0;
                continue;
            }
        };
        cur = (cur << 2) | code;
        valid += 1;
        if valid >= WORD {
            f(i + 1 - WORD, cur);
        }
    }
}

fn pack(allele: usize, minus: bool, pos: usize) -> u32 {
    (allele as u32) << 11 | (minus as u32) << 10 | pos as u32
}
fn unpack(p: u32) -> (usize, bool, usize) {
    ((p >> 11) as usize, (p >> 10) & 1 == 1, (p & 1023) as usize)
}

/// Every allele's 32-mers in both orientations, built once per scheme.
pub(crate) struct SeedIndex {
    forward: Vec<Vec<u8>>,
    reverse: Vec<Vec<u8>>,
    database_bases: usize,
    map: FxMap<u64, (u32, u32)>,
    postings: Vec<u32>,
}

impl SeedIndex {
    /// `alleles` must be unambiguous upper-case sequences shorter than `MAX_ALLELE_LEN`, at most
    /// `MAX_ALLELES` of them; the scheme loader enforces both.
    pub fn build(alleles: Vec<Vec<u8>>) -> SeedIndex {
        debug_assert!(alleles.len() <= MAX_ALLELES);
        let reverse: Vec<Vec<u8>> = alleles.iter().map(|a| revcomp(a)).collect();
        let database_bases = alleles.iter().map(Vec::len).sum();
        let mut pairs: Vec<(u64, u32)> = Vec::with_capacity(database_bases * 2);
        for (i, (f, r)) in alleles.iter().zip(&reverse).enumerate() {
            debug_assert!(f.len() < MAX_ALLELE_LEN);
            kmers(f, |p, km| pairs.push((km, pack(i, false, p))));
            kmers(r, |p, km| pairs.push((km, pack(i, true, p))));
        }
        pairs.sort_unstable();
        let mut map: FxMap<u64, (u32, u32)> = FxMap::default();
        let mut postings = Vec::with_capacity(pairs.len());
        for group in pairs.chunk_by(|a, b| a.0 == b.0) {
            map.insert(group[0].0, (postings.len() as u32, group.len() as u32));
            postings.extend(group.iter().map(|p| p.1));
        }
        SeedIndex {
            forward: alleles,
            reverse,
            database_bases,
            map,
            postings,
        }
    }

    pub fn database_bases(&self) -> usize {
        self.database_bases
    }

    fn get(&self, kmer: u64) -> &[u32] {
        match self.map.get(&kmer) {
            Some(&(s, l)) => &self.postings[s as usize..(s + l) as usize],
            None => &[],
        }
    }

    /// All HSPs of every seeded diagonal. Contigs must be upper case with every base other than
    /// A, C, G and T replaced by N.
    pub fn search(&self, contigs: &[Vec<u8>]) -> Vec<Hsp> {
        let mut diagonals: HashSet<(u32, bool, u32, i64)> = HashSet::new();
        for (ci, c) in contigs.iter().enumerate() {
            kmers(c, |p, km| {
                for &post in self.get(km) {
                    let (a, minus, ap) = unpack(post);
                    diagonals.insert((a as u32, minus, ci as u32, p as i64 - ap as i64));
                }
            });
        }
        let mut sorted: Vec<_> = diagonals.into_iter().collect();
        sorted.sort_unstable();
        let mut out = Vec::new();
        for (allele, minus, contig, diag) in sorted {
            let oriented = if minus {
                &self.reverse[allele as usize]
            } else {
                &self.forward[allele as usize]
            };
            extend_diagonal(
                oriented,
                &contigs[contig as usize],
                diag as isize,
                contig as usize,
                allele as usize,
                minus,
                &mut out,
            );
        }
        out
    }
}

#[cfg(test)]
pub(crate) fn seq(n: usize, seed: u64) -> Vec<u8> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            b"ACGT"[(s >> 33) as usize % 4]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_allele_is_one_full_hsp() {
        let a = seq(300, 1);
        let mut c = seq(500, 2);
        c.splice(100..100, a.iter().copied());
        let mut out = vec![];
        extend_diagonal(&a, &c, 100, 0, 0, false, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(
            (out[0].start, out[0].end, out[0].nident, out[0].alen),
            (100, 400, 300, 300)
        );
    }

    #[test]
    fn terminal_substitution_within_two_bases_of_the_end_is_trimmed() {
        // A mismatch followed by two or fewer matches does not pay for itself under +1/−2.
        for (d, full) in [(0usize, false), (1, false), (2, false), (3, true)] {
            let a = seq(300, 3);
            let mut c = seq(500, 4);
            let mut mutated = a.clone();
            let p = 300 - 1 - d;
            mutated[p] = if a[p] == b'A' { b'C' } else { b'A' };
            c.splice(100..100, mutated.iter().copied());
            let mut out = vec![];
            extend_diagonal(&a, &c, 100, 0, 0, false, &mut out);
            assert_eq!(out.len(), 1);
            assert_eq!(out[0].alen == 300, full, "mismatch {d} bases from the end");
        }
    }

    #[test]
    fn an_insertion_splits_the_diagonal_into_two_partial_hsps() {
        let a = seq(400, 5);
        let mut m = a.clone();
        m.insert(200, b'A');
        let mut c = seq(300, 6);
        c.extend(&m);
        c.extend(seq(300, 7));
        let mut out = vec![];
        extend_diagonal(&a, &c, 300, 0, 0, false, &mut out);
        extend_diagonal(&a, &c, 301, 0, 0, false, &mut out);
        assert!(out.iter().all(|h| h.alen < 400));
        assert!(out.iter().map(|h| h.nident).sum::<u32>() >= 390);
    }

    #[test]
    fn n_is_a_mismatch_and_never_seeds() {
        let a = seq(300, 8);
        let mut c = seq(400, 9);
        let mut m = a.clone();
        m[150] = b'N';
        c.splice(50..50, m.iter().copied());
        let mut out = vec![];
        extend_diagonal(&a, &c, 50, 0, 0, false, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].alen, out[0].nident), (300, 299));
    }

    #[test]
    fn evalue_floor_is_far_below_the_smallest_allowed_partial_hit() {
        // The shortest allele is 249 bases, so a kept hit has at least 125 identical bases.
        assert!(min_score_for_evalue(5_000_000, 2_200_000, 1e-20) < 125);
    }

    #[test]
    fn search_finds_forward_and_reverse_alleles_on_their_strands() {
        let a = seq(300, 10);
        let b = seq(300, 11);
        let index = SeedIndex::build(vec![a.clone(), b.clone()]);
        let mut contig = seq(1000, 12);
        contig.extend(&a);
        contig.extend(seq(1000, 13));
        contig.extend(revcomp(&b));
        let hits: Vec<_> = index
            .search(&[contig])
            .into_iter()
            .filter(|h| h.nident == 300)
            .map(|h| (h.allele, h.minus, h.start, h.end))
            .collect();
        assert_eq!(hits, [(0, false, 1000, 1300), (1, true, 2300, 2600)]);
    }
}
