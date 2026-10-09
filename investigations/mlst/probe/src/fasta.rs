//! Assembly FASTA reading with one validation policy shared by every backend.
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub struct Contig {
    pub name: String,
    pub seq: Vec<u8>,
}

/// Reject malformed input before any backend runs: unreadable/empty file, sequence before a header,
/// empty records, duplicate identifiers, and any character outside the IUPAC nucleotide alphabet.
/// Upper-cases and maps every non-ACGT base to `N`.
pub fn read_assembly(path: &Path) -> Result<Vec<Contig>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut contigs: Vec<Contig> = Vec::new();
    let mut seen = HashSet::new();
    for (n, line) in text.lines().enumerate() {
        if let Some(h) = line.strip_prefix('>') {
            let name = h.split_whitespace().next().unwrap_or("").to_string();
            if name.is_empty() {
                return Err(format!("line {}: empty record identifier", n + 1));
            }
            if !seen.insert(name.clone()) {
                return Err(format!("line {}: duplicate record identifier {name}", n + 1));
            }
            contigs.push(Contig { name, seq: Vec::new() });
        } else if !line.trim().is_empty() {
            let c = contigs.last_mut().ok_or_else(|| format!("line {}: sequence before first header", n + 1))?;
            for b in line.trim().bytes() {
                let u = b.to_ascii_uppercase();
                match u {
                    b'A' | b'C' | b'G' | b'T' => c.seq.push(u),
                    b'N' | b'R' | b'Y' | b'S' | b'W' | b'K' | b'M' | b'B' | b'D' | b'H' | b'V' | b'U' | b'-' => c.seq.push(b'N'),
                    _ => return Err(format!("line {}: invalid nucleotide character {:?}", n + 1, b as char)),
                }
            }
        }
    }
    if contigs.is_empty() {
        return Err("no FASTA records".into());
    }
    if let Some(c) = contigs.iter().find(|c| c.seq.is_empty()) {
        return Err(format!("record {} has no sequence", c.name));
    }
    Ok(contigs)
}
