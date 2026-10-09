# Terobacter

Classify Enterobacter isolate assemblies with one self-contained command.

> **Status: development.** Whole-assembly v1 classification and saved-evidence decisions are implemented. No release version is selected. Production implementation, scientific acceptance and release are separate gates tracked in this repository's issues.

Terobacter reports a species and subspecies call only when the approved rules support one. It shows the nearest-reference evidence either way, so a borderline result is never hidden behind a label.

## Commands

`tero` and `terobacter` are the same program under two names. Use `terobacter` if another installed tool already uses `tero`.

```sh
tero [OPTIONS] <ASSEMBLY>...
tero --version     # terobacter <version>
```

There are no subcommands. Classify a whole assembly with a provisioned reference package and its trusted manifest digest:

```sh
tero --reference-package REFERENCE_DIR --expected-sha256 MANIFEST_SHA256 --threads 4 isolate.fa > isolate.json
```

The default policy is `conservative-assembly-v1`. Native v2 is unavailable in this build; requesting it fails explicitly. Every supplied FASTA record is retained, including plasmids and drafts. JSON records lengths, sequence and input byte hashes, the authenticated package identity, engine profile, measurements and the v1 decision. See [whole-assembly classification](docs/assembly.md) for input rules, partial reports and exit codes.

## Build, test, install

You need a recent stable Rust toolchain and [`just`](https://github.com/casey/just). Native ANIm also requires GNU C++ with C++17 support. On macOS, install Homebrew GCC and set `CXX` to its `g++` executable.

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
cargo run --features dev --bin terobacter-dev -- reference-package <DIR> --expected-sha256 <SHA256> [--named-registry-sha256 <SHA256>] [--lineage-registry-sha256 <SHA256>]
cargo run --features dev --bin terobacter-dev -- saved-evidence <EVIDENCE> --reference-package <DIR> --expected-sha256 <SHA256> [--named-registry-sha256 <SHA256>] [--lineage-registry-sha256 <SHA256>]
```

`saved-evidence` reads frozen 0.2.0 saved evidence and prints the v1 species/subspecies decision as JSON, including raw nearest evidence, competitors, scope assessments and all withholding reasons. It runs no new comparisons. See [the v1 contract](docs/v1-decisions.md) for optional bound method/problem envelopes and the supported API boundary.

`reference-package` authenticates a reference package directory against `--expected-sha256`, the SHA-256 that its `portable.json` must have. A manifest with any other digest is refused before its listing is read. Every file listed in the manifest must be present with the listed length and SHA-256, and no listed path may be a symbolic link. A file in the directory that the manifest does not list is refused. The package must carry a licence manifest with the NCBI submitter-rights disclaimer. `--named-registry-sha256` and `--lineage-registry-sha256` pin the two registry tables to digests you give. Nothing is pinned by default, so a table is checked against a digest only when you supply one.

The source manifest must be exactly the record the package's eleven tables, recipe and skani indexes derive, following the research package builder. The record covers concepts with their parents, references, genomovar definitions, the historical type crosswalk, context records, the cohort summary and every count. Each reference is identified by component and accession, so an accession can be both a primary reference and a genomovar definition member. The reporting cohort is a separate record set. Its membership and evidence flags must agree with the package, and its coverage categories are summarised as written. On success the command prints the package identity, licence, counts, cohort summary and components as JSON, including the SHA-256 of each database.

A package converted from the version 1 format carries the version 1 `portable.json` as an attested conversion record. The verifier checks that the record lists the same package, source manifest, export attestation and assets as the version 2 manifest.

`--reference-package` authenticates the package in the same way, then requires the evidence to be bound to it. The evidence's reference package digest must equal the verified manifest digest, or the digest of the attested version 1 manifest it was converted from. The evidence must cover the whole primary component. Each row must give what the package gives for its reference: the species and subspecies with their display names, the name status of the more specific of the two, the role (`type_anchor` or `diversity`), and no lineage. Contradictory evidence is refused, never corrected. The two flags must be given together. Package binding applies to v1 decisions only; `--policy conservative-assembly-anim-v2` refuses `--reference-package`. Without `--reference-package` the decision reports `reference_package.status` as `not_verified`.

### Development exit codes

| Code | Meaning |
|---|---|
| 0 | Success. |
| 1 | Failure: an unreadable file, invalid saved evidence, an I/O error reading a package, or a listed path that is a symbolic link. |
| 2 | Usage error: an unknown option, a missing argument, a malformed digest, or a flag given without the flag it requires. |
| 3 | Missing: the manifest, or a file it lists, is not in the package. |
| 4 | Altered: a file's length or SHA-256 differs from the manifest, or it is longer than listed. |
| 5 | Truncated: a file is shorter than its listed length. |
| 6 | Incompatible: the package or licence schema is not one this build reads. |
| 7 | Inconsistent: the manifest, a component, a table, an attestation or the licence manifest disagrees with the others or is incomplete, or a listed path is unsafe or repeated. |
| 8 | Unresolved: the saved evidence was searched against another reference package, covers another number of references than the component holds, names a reference the component does not have, or gives a reference a species, subspecies, display name, name status, role or lineage that differs from the package's. |
| 9 | Unexpected: `portable.json`, or a pinned registry table, does not have the SHA-256 given on the command line. |
| 10 | Unlisted: a file in the package directory is not listed in the manifest. |

Every rejection prints `error:` followed by the kind of rejection and a message that names the file or field at fault. It prints no decision.

The reciprocal ANIm development operation measures one pair with the approved maintained MUMmer4 profile. It writes input snapshots, native artifacts and exact integer evidence to a fresh directory:

```sh
cargo run --features dev --bin terobacter-dev -- anim-pair --query QUERY.fa --reference REFERENCE.fa --output NEW_DIRECTORY
```

See [the method amendment](docs/contracts/P1-ANIM-MUMMER4-r1.md) and [P4a-r3 command binding](docs/contracts/P4a-r3.md) for provenance, parser replay, resource limits and remaining acceptance gates.

## Layout

| Path | Role |
|---|---|
| `src/evidence.rs`, `src/annotations.rs`, `src/reference/`, `src/decision.rs` | Pure decision logic. Package bytes arrive through a port, so there is no file, terminal or process code. |
| `src/cli.rs`, `src/cli/assembly.rs`, `src/dev.rs` | Argument parsing, files and exit codes for the public and development commands. |
| `src/bin/` | Thin entry points for `tero`, `terobacter` and `terobacter-dev`. |
| `tests/` | Tests at the decision, command-line and packaging seams, with fixtures. |
| `docs/adr/` | Decisions and their reasons. |
| `CONTEXT.md` | Vocabulary. |

See `AGENTS.md` for contribution rules and `CONTEXT.md` for the terms used here.
