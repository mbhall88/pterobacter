//! Route 1b: the pure-Rust minimap2 translation (`minimap2-pure-rs`), same nomination scheme as route 1.
//! The crate only builds an index from a file, so contigs are written to a temporary FASTA first.
use super::{finish, Diag, Oriented};
use crate::fasta::Contig;
use crate::hsp::Hsp;
use crate::package::Package;
use mm2pure::aligner::Aligner;
use std::collections::HashSet;

pub fn run(pkg: &Package, ori: &Oriented, contigs: &[Contig], preset: &str) -> Result<Vec<Hsp>, String> {
    let tmp = std::env::temp_dir().join(format!("mlst-probe-pure-{}.fa", std::process::id()));
    let mut text = String::new();
    for c in contigs { text.push('>'); text.push_str(&c.name); text.push('\n'); text.push_str(std::str::from_utf8(&c.seq).unwrap()); text.push('\n'); }
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    let built = Aligner::builder().preset(preset).with_cigar().best_n(100).index(tmp.to_str().unwrap()).build();
    let _ = std::fs::remove_file(&tmp);
    let mut aligner = built?;
    aligner.map_opt.pri_ratio = 0.1;
    let mut diags: HashSet<Diag> = HashSet::new();
    for (oid, a) in pkg.alleles.iter().enumerate() {
        for r in aligner.map(&a.seq) {
            let Some(extra) = &r.extra else { continue };
            let minus = r.rev;
            let l = a.seq.len() as i64;
            let mut t = r.rs as i64;
            let mut q = if minus { l - r.qe as i64 } else { r.qs as i64 };
            for &c in &extra.cigar.0 {
                let (len, op) = ((c >> 4) as i64, c & 0xf);
                match op {
                    0 | 7 | 8 => { diags.insert((oid as u32, minus, r.rid as u32, t - q)); t += len; q += len; }
                    1 => q += len,
                    2 | 3 => t += len,
                    _ => {}
                }
            }
        }
    }
    Ok(finish(pkg, ori, contigs, &diags))
}
