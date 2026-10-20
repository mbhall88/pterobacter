//! Pterobacter: standalone Enterobacter isolate classification.
//!
//! One crate, one internal library. The decision logic in [`evidence`], [`reference`],
//! [`panel`], [`decision`] (v1), [`rescue`] (v2), [`report`] (TSV and Markdown) and [`mlst`] is pure, so command-line, browser and GUI
//! front ends can share it. Terminal parsing, files and process exit codes live in [`cli`], and
//! in `dev` for the feature-gated development binary.

pub mod annotations;
pub mod cli;
pub mod decision;
#[cfg(feature = "dev")]
pub mod dev;
pub mod evidence;
pub mod genomovar;
pub mod mlst;
pub mod panel;
pub mod reference;
pub mod report;
pub mod rescue;
pub mod skani;

pub mod anim;

mod hex;

// Reviewed upstream implementations are packaged in this crate. Namespaces are
// adapted mechanically; scripts/check_embedded_sources.py checks the adaptation.
#[doc(hidden)]
#[allow(warnings, clippy::all)]
#[rustfmt::skip]
#[macro_use]
#[path = "../vendor/partitions/src/lib.rs"]
pub mod embedded_partitions;
#[doc(hidden)]
#[allow(warnings, clippy::all)]
#[rustfmt::skip]
#[path = "../vendor/skani/src/lib.rs"]
pub mod embedded_skani;
