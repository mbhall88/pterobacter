//! Reciprocal ANIm measurements, separate from taxonomic decisions.
mod metrics;
pub use metrics::{parse_delta, AlignmentStatus, Metrics};

#[derive(Debug, thiserror::Error)]
pub enum AnimError {
    #[error("invalid ANIm evidence: {0}")]
    Invalid(String),
    #[error("ANIm integer overflow")]
    Overflow,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
mod runtime;
pub use runtime::{
    hash_bytes, hash_file, measure_pair, run_worker_if_requested, verify_pair, ExpectedPair,
    InputBinding, Method, Orientation, PairRecord, PairRequest, Receipt, AMENDMENT, METHOD_PROFILE,
};
