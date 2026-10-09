//! Route 1: minimap2 through the Rust binding. Contigs are indexed, each allele is a query, and the base-level
//! CIGAR of every retained alignment nominates ungapped diagonals for the shared HSP finisher.
use super::{finish, Diag, Oriented};
use crate::fasta::Contig;
use crate::hsp::Hsp;
use crate::package::Package;
use minimap2::{Aligner, Strand};
use std::collections::HashSet;
use std::fmt::Write;

pub fn run(pkg: &Package, ori: &Oriented, contigs: &[Contig], preset: &str, raw: Option<&mut String>) -> Result<Vec<Hsp>, String> {
    let seqs: Vec<Vec<u8>> = contigs.iter().map(|c| c.seq.clone()).collect();
    let ids: Vec<Vec<u8>> = contigs.iter().map(|c| c.name.clone().into_bytes()).collect();
    let b = Aligner::builder();
    let b = match preset {
        "asm5" => b.asm5(),
        "asm10" => b.asm10(),
        "asm20" => b.asm20(),
        other => return Err(format!("unsupported preset {other}")),
    };
    let mut aligner = b.with_cigar().with_index_threads(1).with_seqs_and_ids(&seqs, &ids)?;
    // fixed for all presets (protocol): keep secondary alignments
    aligner.mapopt.best_n = 100;
    aligner.mapopt.pri_ratio = 0.1;
    let mut diags: HashSet<Diag> = HashSet::new();
    let mut raw = raw;
    for (oid, a) in pkg.alleles.iter().enumerate() {
        let maps = aligner.map(&a.seq, false, false, None, None, None).map_err(|e| e.to_string())?;
        for m in maps {
            let minus = matches!(m.strand, Strand::Reverse);
            let Some(aln) = &m.alignment else { continue };
            let Some(cigar) = &aln.cigar else { continue };
            let l = a.seq.len() as i64;
            let mut t = m.target_start as i64;
            let mut q = if minus { l - m.query_end as i64 } else { m.query_start as i64 };
            for &(len, op) in cigar {
                let len = len as i64;
                match op {
                    0 | 7 | 8 => { diags.insert((oid as u32, minus, m.target_id as u32, t - q)); t += len; q += len; }
                    1 => q += len,
                    2 | 3 => t += len,
                    _ => {}
                }
            }
            if let Some(buf) = raw.as_deref_mut() {
                let cs = aln.cigar_str.clone().unwrap_or_default();
                let _ = writeln!(buf, "{}_{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", pkg.alleles[oid].id, oid, l, m.query_start, m.query_end,
                                 if minus { '-' } else { '+' }, m.target_name.as_deref().map(|s| s.as_str()).unwrap_or("?"),
                                 m.target_start, m.target_end, cs);
            }
        }
    }
    Ok(finish(pkg, ori, contigs, &diags))
}
