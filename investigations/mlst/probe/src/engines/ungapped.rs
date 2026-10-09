//! Route D (found necessary by the comparator-rule analysis, see protocol addendum): exact 32-mer seeding of
//! the allele set against the assembly plus the ungapped X-drop extension. No alignment library.
use super::{finish, Diag, Oriented};
use crate::fasta::Contig;
use crate::hsp::{Hsp, WORD};
use crate::package::Package;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

#[derive(Default)]
pub struct Fx(u64);
impl Hasher for Fx {
    fn finish(&self) -> u64 { self.0 }
    fn write(&mut self, bytes: &[u8]) { for &b in bytes { self.0 = (self.0.rotate_left(5) ^ b as u64).wrapping_mul(0x517cc1b727220a95); } }
    fn write_u64(&mut self, i: u64) { self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x517cc1b727220a95); }
}
pub type FxMap<K, V> = HashMap<K, V, BuildHasherDefault<Fx>>;

#[inline]
pub fn code(b: u8) -> Option<u64> {
    match b { b'A' => Some(0), b'C' => Some(1), b'G' => Some(2), b'T' => Some(3), _ => None }
}

/// Calls `f(pos_of_window_start, kmer)` for every window of `k` (<=32) clean bases.
pub fn kmers(seq: &[u8], k: usize, mut f: impl FnMut(usize, u64)) {
    let mask = if k == 32 { u64::MAX } else { (1u64 << (2 * k)) - 1 };
    let (mut cur, mut valid) = (0u64, 0usize);
    for (i, &b) in seq.iter().enumerate() {
        match code(b) {
            Some(c) => { cur = ((cur << 2) | c) & mask; valid += 1; }
            None => { valid = 0; cur = 0; }
        }
        if valid >= k { f(i + 1 - k, cur); }
    }
}

/// Packed posting: allele OID (13 bits) | minus (1 bit) | position (10 bits).
pub fn pack(allele: usize, minus: bool, pos: usize) -> u32 { (allele as u32) << 11 | (minus as u32) << 10 | pos as u32 }
pub fn unpack(p: u32) -> (usize, bool, usize) { ((p >> 11) as usize, (p >> 10) & 1 == 1, (p & 1023) as usize) }

pub struct SeedIndex {
    map: FxMap<u64, (u32, u32)>,
    postings: Vec<u32>,
}

impl SeedIndex {
    pub fn build(pkg: &Package, ori: &Oriented, k: usize) -> SeedIndex {
        let mut pairs: Vec<(u64, u32)> = Vec::with_capacity(pkg.total_bases() * 2);
        for (i, a) in pkg.alleles.iter().enumerate() {
            assert!(a.seq.len() < 1024 && i < (1 << 13));
            kmers(&a.seq, k, |p, km| pairs.push((km, pack(i, false, p))));
            kmers(&ori.rc[i], k, |p, km| pairs.push((km, pack(i, true, p))));
        }
        pairs.sort_unstable();
        let mut map: FxMap<u64, (u32, u32)> = FxMap::default();
        let mut postings = Vec::with_capacity(pairs.len());
        let mut i = 0;
        while i < pairs.len() {
            let mut j = i;
            while j < pairs.len() && pairs[j].0 == pairs[i].0 { postings.push(pairs[j].1); j += 1; }
            map.insert(pairs[i].0, ((postings.len() - (j - i)) as u32, (j - i) as u32));
            i = j;
        }
        SeedIndex { map, postings }
    }
    pub fn get(&self, kmer: u64) -> &[u32] {
        match self.map.get(&kmer) { Some(&(s, l)) => &self.postings[s as usize..(s + l) as usize], None => &[] }
    }
}

pub fn run(pkg: &Package, ori: &Oriented, contigs: &[Contig]) -> Vec<Hsp> {
    let index = SeedIndex::build(pkg, ori, WORD);
    let mut diags: HashSet<Diag> = HashSet::new();
    for (ci, c) in contigs.iter().enumerate() {
        kmers(&c.seq, WORD, |p, km| {
            for &post in index.get(km) {
                let (a, minus, ap) = unpack(post);
                diags.insert((a as u32, minus, ci as u32, p as i64 - ap as i64));
            }
        });
    }
    finish(pkg, ori, contigs, &diags)
}
