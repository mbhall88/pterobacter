# Decision logic is pure and front ends are thin

Status: accepted, 9 October 2026.

## Context

The same decisions must be reachable from the command line now and, possibly, from a browser or GUI later. A later front end can only reuse them if they do not depend on a terminal, the filesystem or the process.

## Decision

`src/evidence.rs`, `src/reference.rs` and `src/decision.rs` take text and values and return values. They never read files, parse arguments, print, or exit. `src/reference.rs` reads package bytes only through the `PackageSource` port that a front end implements. `src/cli.rs` and, for the development binary, `src/dev.rs` own all of that and call the pure modules.

The seam for tests is `SavedEvidence::from_json`, `verify_package` and `decide`, plus the executables as processes. Tests do not reach into private helpers.

## Consequences

- Saved evidence is passed as text, so a browser front end can supply it without a filesystem.
- Percentages are held as integer hundredths and compared exactly, because skani reports two decimals and the frozen rules use exactly those decimals. Evidence with more than two decimals is rejected, not rounded. That check reads the number's literal text, so `serde_json` is built with `arbitrary_precision`; read as an f64, `95.999999999999999` would become 96.0 and pass the species gate.
- Reference package authentication sits behind the same seam. `decide` refuses evidence that is not bound to the package it is given, and reports the reference as not verified when no package is given.
- Later tickets add the remaining v1 checks and the subspecies rank behind the same seam.
