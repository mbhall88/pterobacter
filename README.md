# Terobacter

Classify Enterobacter isolate assemblies with one self-contained command.

> **Status: development.** Only a tracer exists. `tero <assembly>` does not classify yet, and no release version is selected. Production implementation, scientific acceptance and release are separate gates tracked in this repository's issues.

Terobacter reports a species and subspecies call only when the approved rules support one. It shows the nearest-reference evidence either way, so a borderline result is never hidden behind a label.

## Commands

`tero` and `terobacter` are the same program under two names. Use `terobacter` if another installed tool already uses `tero`.

```sh
tero [OPTIONS] <ASSEMBLY>...
tero --version     # terobacter <version>
```

There are no subcommands. Whole-assembly classification is not implemented yet: running it now exits with status 1 and says so. The public command does not take a reference package yet.

## Build, test, install

You need a recent stable Rust toolchain and [`just`](https://github.com/casey/just).

```sh
just test            # tests as a default build, then with the development feature
just lint            # clippy with warnings denied
just install-check   # a default install exposes only tero and terobacter
cargo install --path .
```

Without `just`, `cargo test` and `cargo test --all-features` do the same job.

## Development binary

Evaluation-only operations live in `terobacter-dev`. It is built only on request and is never part of a default build or install.

```sh
cargo run --features dev --bin terobacter-dev -- saved-evidence tests/fixtures/v1-species-assigned/evidence.json
cargo run --features dev --bin terobacter-dev -- reference-package <DIR>
cargo run --features dev --bin terobacter-dev -- saved-evidence <EVIDENCE> --reference-package <DIR>
```

`saved-evidence` reads a frozen 0.2.0 saved-evidence file and prints its v1 species call as JSON. The v1 decision is a tracer: it applies the species rule only, and reports the subspecies as not evaluated.

`reference-package` authenticates a reference package directory. Every file listed in its `portable.json` must be present with the listed length and SHA-256. Both components (`primary_whole` and `genomovar_chromosome`) must be complete, and the package must carry a licence manifest with the NCBI submitter-rights disclaimer. On success it prints the package identity, licence and components as JSON, including the SHA-256 of each database.

`--reference-package` authenticates the package first, then requires the evidence to be bound to it: the evidence's reference package digest must equal the verified manifest digest. Without the flag the decision reports `reference_package.status` as `not_verified`.

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Success. |
| 1 | Failure: an unreadable file, invalid saved evidence, or an I/O error reading a package. |
| 2 | Usage error: an unknown option or a missing argument. |
| 3 | Missing: the manifest, or a file it lists, is not in the package. |
| 4 | Altered: a file's length or SHA-256 differs from the manifest, or it is longer than listed. |
| 5 | Truncated: a file is shorter than its listed length. |
| 6 | Incompatible: the package or licence schema is not one this build reads. |
| 7 | Incomplete: the manifest, a component, the licence manifest or a listed path is incomplete or unsafe. |
| 8 | Not bound: the saved evidence was searched against a different reference package. |

Every rejection prints `error:` followed by a message that names the file or field at fault, and prints no decision.

## Layout

| Path | Role |
|---|---|
| `src/evidence.rs`, `src/reference.rs`, `src/decision.rs` | Pure logic. Package bytes arrive through a port, so there is no file, terminal or process code. |
| `src/cli.rs`, `src/dev.rs` | Argument parsing, files and exit codes for the public and development commands. |
| `src/bin/` | Thin entry points for `tero`, `terobacter` and `terobacter-dev`. |
| `tests/` | Tests at the decision, command-line and packaging seams, with fixtures. |
| `docs/adr/` | Decisions and their reasons. |
| `CONTEXT.md` | Vocabulary. |

See `AGENTS.md` for contribution rules and `CONTEXT.md` for the terms used here.
