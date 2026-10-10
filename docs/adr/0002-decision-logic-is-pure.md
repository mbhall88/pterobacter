# Decision logic is pure and front ends are thin

Status: accepted, 9 October 2026.

## Context

The same decisions must be reachable from the command line now and, possibly, from a browser or GUI later. A later front end can only reuse them if they do not depend on a terminal, the filesystem or the process.

## Decision

`src/evidence.rs`, `src/annotations.rs`, `src/reference/`, `src/decision.rs`, `src/panel.rs` and `src/rescue/` take text and values and return values. They never read files, parse arguments, print, or exit. `src/reference/` reads package bytes only through the `PackageSource` port that a front end implements. `src/cli.rs` and, for the development binary, `src/dev.rs` own all of that and call the pure modules.

The seam for tests is `SavedEvidence::from_json`, `verify_package`, `decide` and `decide_bound`, plus the executables as processes. For v2 it is `Panel::from_json`, `AnimRecords::from_jsonl`, `RescueInput` and its `decide` and `plan`. Tests normally do not reach into private helpers. A narrow exception permits module-local tests of final-publication integrity routing in `src/cli.rs`: `record_failure` and `retains_assignment` distinguish a supplementary failure from an integrity failure so retained parent calls still receive the final package check. These guards remain covered by public workflow and executable regressions; the exception does not move scientific decisions into the front end.

## Consequences

- Saved evidence is passed as text, so a browser front end can supply it without a filesystem.
- Percentages are held as integer hundredths and compared exactly, because skani reports two decimals and the frozen rules use exactly those decimals. Evidence with more than two decimals is rejected, not rounded. That check reads the number's literal text, so `serde_json` is built with `arbitrary_precision`; read as an f64, `95.999999999999999` would become 96.0 and pass the species gate.
- Reference package authentication sits behind the same seam. `decide_bound` refuses evidence that is not bound to the package it is given: one with another manifest digest, a reference the package does not have, or a species, subspecies, display name, name status, role or lineage that differs from the package's. `decide` reports the reference as not verified, and is the replay path for evidence without a package.
- Issue #5 adds the remaining v1 checks and subspecies rank behind this seam, with bound annotations supplied through `SavedEvidence::with_annotations`. Package authentication stays a separate responsibility.
- Issue #9 adds the v2 rescue and the dry rescue plan behind the same seam. ANIm records and the primary panel arrive as text too, and the plan runs no alignment.
