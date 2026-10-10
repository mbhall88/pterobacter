//! A synthetic seven-locus scheme package in the historical MLST manifest format.
use super::dna;
use super::package::sha256_hex;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read};
use terobacter::mlst::{LIMITATION, LOCI};
use terobacter::reference::PackageSource;

pub const ALLELE_LEN: usize = 300;

/// Alleles 1 and 2 at each locus; allele 2 differs from allele 1 at base 150. Allele 3 at dnaA
/// differs from allele 1 at bases 100 and 200. Profiles: ST5 is all 1s, ST6 is dnaA 2 and
/// otherwise 1s.
pub struct SyntheticScheme {
    pub alleles: BTreeMap<(usize, u32), Vec<u8>>,
    pub profiles: Vec<(String, [u32; 7])>,
    pub files: BTreeMap<String, Vec<u8>>,
}

fn substitute(seq: &mut [u8], at: usize) {
    seq[at] = if seq[at] == b'A' { b'C' } else { b'A' };
}

impl SyntheticScheme {
    pub fn new() -> Self {
        let mut alleles = BTreeMap::new();
        for locus in 0..LOCI.len() {
            let first = dna(ALLELE_LEN, 100 + locus as u64);
            let mut second = first.clone();
            substitute(&mut second, 150);
            alleles.insert((locus, 1), first.clone());
            alleles.insert((locus, 2), second);
            if locus == 0 {
                let mut third = first;
                substitute(&mut third, 100);
                substitute(&mut third, 200);
                alleles.insert((locus, 3), third);
            }
        }
        let profiles = vec![("5".into(), [1; 7]), ("6".into(), [2, 1, 1, 1, 1, 1, 1])];
        let mut scheme = SyntheticScheme {
            alleles,
            profiles,
            files: BTreeMap::new(),
        };
        scheme.render();
        scheme
    }

    pub fn allele(&self, locus: usize, id: u32) -> Vec<u8> {
        self.alleles[&(locus, id)].clone()
    }

    /// Rewrite the data files and the manifest from `alleles` and `profiles`.
    pub fn render(&mut self) {
        self.files.clear();
        let mut hashes = serde_json::Map::new();
        let mut counts = serde_json::Map::new();
        for (li, locus) in LOCI.iter().enumerate() {
            let mut fasta = Vec::new();
            let mut count = 0;
            for ((l, id), seq) in &self.alleles {
                if *l == li {
                    fasta.extend(format!(">{locus}_{id}\n").bytes());
                    fasta.extend(seq);
                    fasta.push(b'\n');
                    count += 1;
                }
            }
            let path = format!("pubmlst/ecloacae/{locus}.tfa");
            hashes.insert(path.clone(), sha256_hex(&fasta).into());
            counts.insert((*locus).into(), count.into());
            self.files.insert(path, fasta);
        }
        let mut table = format!("ST\t{}\tclonal_complex\n", LOCI.join("\t"));
        for (st, profile) in &self.profiles {
            let ids: Vec<String> = profile.iter().map(u32::to_string).collect();
            table.push_str(&format!("{st}\t{}\t\n", ids.join("\t")));
        }
        let path = "pubmlst/ecloacae/ecloacae.txt".to_string();
        hashes.insert(path.clone(), sha256_hex(table.as_bytes()).into());
        self.files.insert(path, table.into_bytes());
        let manifest = json!({
            "schema_version": 1,
            "package_id": "synthetic-mlst-pre2025",
            "scheme": "ecloacae",
            "scheme_description": "Synthetic seven-locus test scheme",
            "loci": LOCI,
            "submission_cutoff": "2024-12-31",
            "retrieved_utc": "2026-10-10T00:00:00+00:00",
            "sequence_state": "synthetic test data",
            "complete_through_cutoff_independently_established": false,
            "data_terms_url": "https://pubmlst.org/terms-conditions",
            "limitation": LIMITATION,
            "profile_count": self.profiles.len(),
            "allele_counts": counts,
            "data_sha256": hashes,
        });
        self.set_manifest(&manifest);
    }

    pub fn manifest(&self) -> Value {
        serde_json::from_slice(&self.files["manifest.json"]).unwrap()
    }

    pub fn set_manifest(&mut self, manifest: &Value) {
        self.files.insert(
            "manifest.json".into(),
            serde_json::to_vec_pretty(manifest).unwrap(),
        );
    }

    pub fn manifest_sha256(&self) -> String {
        sha256_hex(&self.files["manifest.json"])
    }

    pub fn write_to(&self, dir: &std::path::Path) {
        for (path, bytes) in &self.files {
            let target = dir.join(path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, bytes).unwrap();
        }
    }

    /// An assembly with each piece between random flanks on one record.
    pub fn assembly(&self, pieces: &[Vec<u8>]) -> Vec<u8> {
        let mut seq = dna(2000, 1);
        for (i, piece) in pieces.iter().enumerate() {
            seq.extend(piece);
            seq.extend(dna(1500, 10 + i as u64));
        }
        seq
    }

    /// The seven allele 1 sequences, the reverse complement used at odd loci.
    pub fn st5_pieces(&self) -> Vec<Vec<u8>> {
        (0..LOCI.len())
            .map(|l| {
                let a = self.allele(l, 1);
                if l % 2 == 1 {
                    revcomp(&a)
                } else {
                    a
                }
            })
            .collect()
    }
}

pub fn revcomp(s: &[u8]) -> Vec<u8> {
    s.iter()
        .rev()
        .map(|&b| match b {
            b'A' => b'T',
            b'C' => b'G',
            b'G' => b'C',
            _ => b'A',
        })
        .collect()
}

impl PackageSource for SyntheticScheme {
    fn open(&self, path: &str) -> io::Result<Option<Box<dyn Read + '_>>> {
        Ok(self
            .files
            .get(path)
            .map(|b| Box::new(b.as_slice()) as Box<dyn Read>))
    }

    fn inventory(&self) -> io::Result<BTreeSet<String>> {
        Ok(self.files.keys().cloned().collect())
    }
}
