# One crate, two command names, and a feature-gated development binary

Status: accepted, 9 October 2026 (decided by Michael).

## Context

Terobacter needs a user command, plus evaluation-only operations that must never reach users: saved-evidence classification, single ANIm pairs, dry plans and genomovar diagnostics. The tool must also work as one self-contained binary, and some people already have another tool called `tero`.

## Decision

- A single `terobacter` crate with an internal library. The library is not published separately.
- Two executables from the same code, `tero` and `terobacter`. Both call `terobacter::cli::run_public`. `--version` prints `terobacter <version>` under either name.
- The public command is `tero [OPTIONS] <ASSEMBLY>...` with no subcommands.
- Evaluation-only operations live in a third binary, `terobacter-dev`, declared with `required-features = ["dev"]`. A default build or `cargo install` skips it. It is built from the same code and commit as the public binaries.
- The crate sets `publish = false` until a public release is decided.

## Consequences

- `tests/layout.rs` fails if the development binary loses its feature gate or a third default binary appears.
- Adding an evaluation operation means adding a subcommand to `terobacter-dev`, not to `tero`.
- All development code lives in `src/dev.rs`, which `lib.rs` declares behind `#[cfg(feature = "dev")]`.


## Amendment, 15 October 2026

Issue #39 and its reviewed distribution contract authorize public
`ptero database install VERSION`, `database list` and `database verify VERSION`.
Assembly classification remains `ptero [OPTIONS] <ASSEMBLY>...`; evaluation
operations remain confined to the development binary. The public executable
names are now `ptero` and `pterobacter`, with the internal library still named
`terobacter`. Managed database transport, files and CLI parsing live in
`src/cli/database/`; scientific loaders remain pure. See
[database management](../databases.md) for trust, lifecycle and filesystem rules.
