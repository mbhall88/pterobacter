//! The skani index is a count followed by one entry per sketched sequence: a length-prefixed
//! label, then the byte offset and size of that sequence's sketch in `sketches.db`.

use std::collections::BTreeSet;

use super::{inconsistent, PackageError};

const COUNT_BYTES: usize = 8;
const MIN_ENTRY_BYTES: u64 = 24;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Entry {
    pub(super) label: String,
    pub(super) offset: u64,
    pub(super) bytes: u64,
}

pub(super) fn parse(bytes: &[u8], sketches_len: u64) -> Result<Vec<Entry>, PackageError> {
    let mut reader = Reader { bytes, at: 0 };
    let count = reader.u64()?;
    let remaining = (bytes.len() - COUNT_BYTES) as u64;
    if count == 0 || count > remaining / MIN_ENTRY_BYTES {
        return Err(inconsistent(
            "the skani index has an impossible sequence count",
        ));
    }

    let mut entries = Vec::with_capacity(count as usize);
    let mut labels = BTreeSet::new();
    let mut next_offset = 0u64;
    for _ in 0..count {
        let name_len = reader.u64()?;
        let label = std::str::from_utf8(reader.take(name_len)?)
            .map_err(|_| inconsistent("the skani index has a label that is not UTF-8"))?
            .to_string();
        let offset = reader.u64()?;
        let size = reader.u64()?;
        if label.is_empty() {
            return Err(inconsistent("the skani index has an empty label"));
        }
        if !labels.insert(label.clone()) {
            return Err(inconsistent(format!("the skani index repeats {label}")));
        }
        if offset != next_offset || size == 0 {
            return Err(inconsistent(format!(
                "the skani index places {label} at an unexpected offset or size"
            )));
        }
        next_offset = next_offset
            .checked_add(size)
            .ok_or_else(|| inconsistent("the skani index sizes overflow"))?;
        entries.push(Entry {
            label,
            offset,
            bytes: size,
        });
    }
    if reader.at != bytes.len() {
        return Err(inconsistent("the skani index has trailing bytes"));
    }
    if next_offset != sketches_len {
        return Err(inconsistent(
            "the skani index sizes do not cover sketches.db",
        ));
    }
    Ok(entries)
}

pub(super) fn check_membership(
    entries: &[Entry],
    expected: &BTreeSet<&str>,
) -> Result<(), PackageError> {
    let indexed: BTreeSet<&str> = entries.iter().map(|entry| entry.label.as_str()).collect();
    if let Some(unknown) = indexed.iter().find(|label| !expected.contains(**label)) {
        return Err(inconsistent(format!(
            "the skani index names {unknown}, which is not a component reference"
        )));
    }
    if let Some(missing) = expected.iter().find(|label| !indexed.contains(**label)) {
        return Err(inconsistent(format!(
            "the skani index does not name {missing}"
        )));
    }
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn u64(&mut self) -> Result<u64, PackageError> {
        let raw: [u8; 8] = self.take(8)?.try_into().map_err(|_| truncated())?;
        Ok(u64::from_le_bytes(raw))
    }

    fn take(&mut self, len: u64) -> Result<&[u8], PackageError> {
        let len = usize::try_from(len).map_err(|_| truncated())?;
        let end = self
            .at
            .checked_add(len)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(truncated)?;
        let slice = &self.bytes[self.at..end];
        self.at = end;
        Ok(slice)
    }
}

fn truncated() -> PackageError {
    inconsistent("the skani index ends before its entries do")
}
