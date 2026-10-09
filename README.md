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

There are no subcommands. Whole-assembly classification is not implemented yet: running it now exits with status 1 and says so.

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
```

This reads a frozen 0.2.0 saved-evidence file and prints its v1 species call as JSON. The v1 decision is a tracer: it applies the species rule only, and reports the subspecies as not evaluated.

## Layout

| Path | Role |
|---|---|
| `src/evidence.rs`, `src/decision.rs` | Pure decision logic. No files, terminal or process code. |
| `src/cli.rs` | Argument parsing, files and exit codes. |
| `src/bin/` | Thin entry points for `tero`, `terobacter` and `terobacter-dev`. |
| `tests/` | Tests at the decision, command-line and packaging seams, with fixtures. |
| `docs/adr/` | Decisions and their reasons. |
| `CONTEXT.md` | Vocabulary. |

See `AGENTS.md` for contribution rules and `CONTEXT.md` for the terms used here.
