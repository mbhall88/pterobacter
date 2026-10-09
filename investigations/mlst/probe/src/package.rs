//! Scheme package loader: seven locus FASTAs plus the ST profile table (mlst's `<scheme>/<scheme>.txt` layout).
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub const LOCI: [&str; 7] = ["dnaA", "fusA", "gyrB", "leuS", "pyrG", "rplB", "rpoB"];

pub struct Allele {
    pub locus: usize,
    pub id: String,
    pub seq: Vec<u8>,
}

pub struct Package {
    /// Index in this vector is the BLAST database OID (loci in alphabetical file order, file order within a locus).
    pub alleles: Vec<Allele>,
    /// `a/b/c/d/e/f/g` signature (missing/0 as `-`) to ST, later rows overriding earlier ones as in mlst.
    pub profiles: HashMap<String, String>,
}

impl Package {
    pub fn load(dir: &Path) -> Result<Package, String> {
        let mut alleles = Vec::new();
        for (li, locus) in LOCI.iter().enumerate() {
            let path = dir.join(format!("{locus}.tfa"));
            let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let (mut id, mut seq): (Option<String>, Vec<u8>) = (None, Vec::new());
            let mut flush = |id: &mut Option<String>, seq: &mut Vec<u8>| {
                if let Some(i) = id.take() {
                    alleles.push(Allele { locus: li, id: i, seq: std::mem::take(seq) });
                }
            };
            for line in text.lines() {
                if let Some(h) = line.strip_prefix('>') {
                    flush(&mut id, &mut seq);
                    let name = h.split_whitespace().next().unwrap_or("");
                    let (l, n) = name.rsplit_once(['_', '-']).ok_or_else(|| format!("bad allele header {name}"))?;
                    if l != *locus {
                        return Err(format!("allele {name} in {locus}.tfa"));
                    }
                    id = Some(n.to_string());
                } else {
                    seq.extend(line.trim().bytes().map(|b| b.to_ascii_uppercase()));
                }
            }
            flush(&mut id, &mut seq);
        }
        let table = fs::read_to_string(dir.join("ecloacae.txt")).map_err(|e| e.to_string())?;
        let mut profiles = HashMap::new();
        for line in table.lines().skip(1) {
            let cols: Vec<&str> = line.split('\t').collect();
            let sig: Vec<&str> = (1..=7).map(|i| match cols.get(i) { Some(c) if !c.is_empty() && *c != "0" => *c, _ => "-" }).collect();
            profiles.insert(sig.join("/"), cols[0].to_string());
        }
        Ok(Package { alleles, profiles })
    }

    pub fn total_bases(&self) -> usize {
        self.alleles.iter().map(|a| a.seq.len()).sum()
    }
}
