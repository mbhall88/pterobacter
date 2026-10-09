//! Terobacter: standalone Enterobacter isolate classification.
//!
//! One crate, one internal library. The decision logic in [`evidence`], [`reference`],
//! [`panel`], [`decision`] (v1) and [`rescue`] (v2) is pure, so command-line, browser and GUI
//! front ends can share it. Terminal parsing, files and process exit codes live in [`cli`], and
//! in `dev` for the feature-gated development binary.

pub mod annotations;
pub mod cli;
pub mod decision;
#[cfg(feature = "dev")]
pub mod dev;
pub mod evidence;
pub mod panel;
pub mod reference;
pub mod rescue;
pub mod skani;

pub mod anim;
