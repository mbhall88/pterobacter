//! Ungapped high-scoring segment pairs as produced by blastn `-ungapped -word_size 32` with megablast
//! scoring (+1/-2), and the shared seed-and-extend routine that candidate engines use to finish a
//! candidate diagonal into BLAST-comparable HSPs.

pub const WORD: usize = 32;
pub const REWARD: i32 = 1;
pub const PENALTY: i32 = -2;
/// Raw ungapped X-drop. blastn documents 20 bits but its raw conversion is not stated; comparator hits bracket it
/// to 12..=14 (11 and 15 each break one hit per fixture), so the midpoint is used. See protocol addendum B.
pub const XDROP: i32 = 13;

#[derive(Clone, Debug)]
pub struct Hsp {
    pub contig: usize,
    /// Allele index = BLAST database OID.
    pub allele: usize,
    /// Half-open forward-strand contig interval.
    pub start: usize,
    pub end: usize,
    pub minus: bool,
    pub nident: u32,
    pub alen: u32,
    pub score: i32,
}

pub fn revcomp(s: &[u8]) -> Vec<u8> {
    s.iter().rev().map(|&b| match b { b'A' => b'T', b'C' => b'G', b'G' => b'C', b'T' => b'A', _ => b'N' }).collect()
}

#[inline]
fn is_match(a: u8, b: u8) -> bool {
    a == b && a != b'N'
}

/// Finish one diagonal. `allele` is oriented to match the forward contig; `diag` = contig_pos - allele_pos.
/// Every maximal exact run of at least WORD bases not already inside an HSP seeds an X-drop extension
/// in both directions that keeps the highest-scoring end (first maximum, as the extension is strict).
pub fn extend_diagonal(allele: &[u8], contig: &[u8], diag: isize, contig_idx: usize, allele_idx: usize, minus: bool, out: &mut Vec<Hsp>) {
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
        // extend right from the end of the run
        let (mut best, mut cur, mut right) = (0, 0, run_end);
        let mut i = run_end;
        while i < q_hi {
            cur += if is_match(allele[i], at(i)) { REWARD } else { PENALTY };
            i += 1;
            if cur > best {
                best = cur;
                right = i;
            } else if best - cur >= XDROP {
                break;
            }
        }
        // extend left from the start of the run
        let (mut lbest, mut lcur, mut left) = (0, 0, run_start);
        let mut i = run_start;
        while i > q_lo {
            i -= 1;
            lcur += if is_match(allele[i], at(i)) { REWARD } else { PENALTY };
            if lcur > lbest {
                lbest = lcur;
                left = i;
            } else if lbest - lcur >= XDROP {
                break;
            }
        }
        let alen = right - left;
        let nident = (left..right).filter(|&i| is_match(allele[i], at(i))).count() as u32;
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

/// Smallest raw score passing `-evalue 1E-20` for a contig of `m` bases against a database of `n` bases
/// (ungapped +1/-2 statistics: lambda 1.28, K 0.46, H 0.85, with the usual edge-effect correction).
pub fn min_score_for_evalue(m: usize, n: usize, evalue: f64) -> i32 {
    let (lambda, k, h) = (1.28f64, 0.46f64, 0.85f64);
    let (m, n) = (m as f64, n as f64);
    let l = ((k * m * n).ln() / h).max(0.0);
    let (me, ne) = ((m - l).max(1.0), (n - l).max(1.0));
    ((k * me * ne / evalue).ln() / lambda).ceil() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seq(n: usize, seed: u64) -> Vec<u8> {
        let mut s = seed;
        (0..n).map(|_| { s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); b"ACGT"[(s >> 33) as usize % 4] }).collect()
    }

    #[test]
    fn exact_allele_is_one_full_hsp() {
        let a = seq(300, 1);
        let mut c = seq(500, 2);
        c.splice(100..100, a.iter().copied());
        let mut out = vec![];
        extend_diagonal(&a, &c, 100, 0, 0, false, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].start, out[0].end, out[0].nident, out[0].alen), (100, 400, 300, 300));
    }

    #[test]
    fn terminal_substitution_within_two_bases_of_the_end_is_trimmed() {
        // +1/-2 scoring: a mismatch followed by 2 or fewer matches does not pay for itself (strict improvement required).
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
        // smallest allele is 249 bases, mincov 50% => at least 125 identical bases.
        assert!(min_score_for_evalue(5_000_000, 2_200_000, 1e-20) < 125);
    }
}
