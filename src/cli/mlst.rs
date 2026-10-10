//! Optional historical MLST for `tero <assembly>`: opens the scheme package once, types each
//! validated assembly and keeps a failure explicit without touching the classification.
use crate::cli::Directory;
use crate::evidence::sha256_hex;
use crate::mlst::{type_assembly, Record, Scheme};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Exit code when MLST was requested and did not complete for some input.
pub const MLST_FAILED: u8 = 6;

/// The report field when MLST was not requested.
pub fn not_requested() -> Value {
    json!({"requested": false, "status": "not_requested"})
}

fn failed(stage: &str, error: &str) -> Value {
    json!({"requested": true, "status": "failed", "stage": stage, "error": error,
        "state": null, "sequence_type": null, "engine": crate::mlst::engine()})
}

/// FASTA records as the engine sees them: the first header word and the sequence.
pub fn parse_records(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut reader =
        needletail::parse_fastx_reader(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut records = Vec::new();
    while let Some(record) = reader.next() {
        let record = record.map_err(|e| e.to_string())?;
        let header = std::str::from_utf8(record.id()).map_err(|e| e.to_string())?;
        let id = header.split_whitespace().next().unwrap_or("").to_owned();
        records.push((id, record.seq().into_owned()));
    }
    Ok(records)
}

/// A requested scheme package, opened on first use and rechecked before reports leave.
pub struct Mlst {
    directory: PathBuf,
    expected_sha256: String,
    scheme: OnceLock<Result<Scheme, String>>,
}

impl Mlst {
    pub fn new(directory: &Path, expected_sha256: &str) -> Self {
        Self {
            directory: directory.to_owned(),
            expected_sha256: expected_sha256.into(),
            scheme: OnceLock::new(),
        }
    }

    /// The `mlst` report field when an earlier stage stopped the input before MLST could run.
    pub fn not_run(&self, stage: &str) -> Value {
        failed(
            stage,
            &format!("the {stage} stage failed, so MLST did not run"),
        )
    }

    /// The `mlst` report field for one input. `bytes` is `None` when the input was not validated.
    pub fn report(&self, bytes: Option<&[u8]>) -> Value {
        let Some(bytes) = bytes else {
            return self.not_run("input");
        };
        let (directory, expected) = (&self.directory, &self.expected_sha256);
        let scheme = self.scheme.get_or_init(|| {
            Scheme::verify(&Directory(directory.clone()), expected).map_err(|e| e.to_string())
        });
        let scheme = match scheme {
            Ok(scheme) => scheme,
            Err(error) => return failed("package", error),
        };
        let records = match parse_records(bytes) {
            Ok(records) => records,
            Err(error) => return failed("input", &error),
        };
        let records: Vec<Record> = records.iter().map(|(id, seq)| Record { id, seq }).collect();
        match type_assembly(scheme, &sha256_hex(bytes), &records) {
            Ok(mut record) => {
                record["requested"] = true.into();
                record
            }
            Err(error) => {
                let stage = match error {
                    crate::mlst::MlstError::Input(_) => "input",
                    _ => "typing",
                };
                failed(stage, &error.to_string())
            }
        }
    }

    /// Reject a scheme package changed during the run.
    pub fn finish(&self) -> Result<(), String> {
        match self.scheme.get() {
            Some(Ok(scheme)) => scheme
                .recheck(&Directory(self.directory.clone()))
                .map_err(|e| e.to_string()),
            _ => Ok(()),
        }
    }
}

/// Mark completed MLST records failed after a failed package recheck.
pub fn invalidate(report: &mut Value, error: &str) {
    if report["mlst"]["status"] == "complete" {
        report["mlst"] = failed("package_recheck", error);
    }
}
