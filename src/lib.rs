//! Terobacter: standalone Enterobacter isolate classification.
//!
//! One crate, one internal library. The decision logic in [`evidence`] and [`decision`] is
//! pure, so command-line, browser and GUI front ends can share it. Terminal parsing, files
//! and process exit codes live in [`cli`].

pub mod cli;
pub mod decision;
pub mod evidence;
