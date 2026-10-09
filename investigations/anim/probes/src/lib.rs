use std::{collections::HashSet, fs, path::Path};

pub type Fasta = Vec<(String, Vec<u8>)>;

pub fn read_fasta(path: &Path) -> Result<Fasta, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut records: Fasta = Vec::new();
    let mut ids = HashSet::new();
    for line in text.lines().map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(header) = line.strip_prefix('>') {
            if records.last().is_some_and(|r| r.1.is_empty()) {
                return Err("empty FASTA record".into());
            }
            let id = header.split_whitespace().next().ok_or("empty FASTA ID")?;
            if id.contains('\0') || !ids.insert(id.to_owned()) {
                return Err("duplicate or invalid FASTA ID".into());
            }
            records.push((id.to_owned(), Vec::new()));
        } else {
            let record = records.last_mut().ok_or("sequence before FASTA header")?;
            for base in line.bytes().map(|b| b.to_ascii_uppercase()) {
                if !b"ACGTRYSWKMBDHVN".contains(&base) {
                    return Err("invalid nucleotide alphabet".into());
                }
                record.1.push(base);
            }
        }
    }
    if records.is_empty() || records.last().is_some_and(|r| r.1.is_empty()) {
        return Err("empty FASTA file or record".into());
    }
    if records.iter().any(|r| r.1.len() > i32::MAX as usize) {
        return Err("record exceeds binding's i32 length".into());
    }
    Ok(records)
}
