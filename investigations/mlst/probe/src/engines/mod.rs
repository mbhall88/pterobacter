//! Candidate engines. Each only nominates candidate diagonals or exact matches; `finish` turns a diagonal
//! into BLAST-comparable ungapped HSPs with one shared routine, so the engines differ in discovery only.
use crate::fasta::Contig;
use crate::hsp::{extend_diagonal, revcomp, Hsp};
use crate::package::Package;
use std::collections::HashSet;

pub mod ungapped;
#[cfg(feature = "engine-minimap2")]
pub mod minimap2;
#[cfg(feature = "engine-minimap2-pure")]
pub mod minimap2_pure;
#[cfg(any(feature = "engine-exact-bio", feature = "engine-exact-blockaligner"))]
pub mod exact;

/// Allele sequences in both orientations (index = allele OID).
pub struct Oriented {
    pub rc: Vec<Vec<u8>>,
}

impl Oriented {
    pub fn new(pkg: &Package) -> Self {
        Oriented { rc: pkg.alleles.iter().map(|a| revcomp(&a.seq)).collect() }
    }
    pub fn seq<'a>(&'a self, pkg: &'a Package, allele: usize, minus: bool) -> &'a [u8] {
        if minus { &self.rc[allele] } else { &pkg.alleles[allele].seq }
    }
}

/// A candidate diagonal: allele (OID), orientation, contig index and contig_pos - oriented_allele_pos.
pub type Diag = (u32, bool, u32, i64);

pub fn finish(pkg: &Package, ori: &Oriented, contigs: &[Contig], diags: &HashSet<Diag>) -> Vec<Hsp> {
    let mut sorted: Vec<&Diag> = diags.iter().collect();
    sorted.sort();
    let mut out = Vec::new();
    for &(allele, minus, contig, diag) in sorted {
        extend_diagonal(ori.seq(pkg, allele as usize, minus), &contigs[contig as usize].seq, diag as isize, contig as usize, allele as usize, minus, &mut out);
    }
    out
}
