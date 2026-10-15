# Development commands and source layout

Use this guide when working on the implementation or replaying saved evidence. The public `ptero` command takes assembly paths directly; the operations below belong to the optional development binary. For repository checks, see [Contributing](../CONTRIBUTING.md).


Evaluation-only operations live in `pterobacter-dev`. It is built only on request and is never part of a default build or install.

```sh
cargo run --features dev --bin pterobacter-dev -- saved-evidence <EVIDENCE>
cargo run --features dev --bin pterobacter-dev -- reference-package <DIR> --expected-sha256 <SHA256> [--named-registry-sha256 <SHA256>] [--lineage-registry-sha256 <SHA256>]
cargo run --features dev --bin pterobacter-dev -- saved-evidence <EVIDENCE> --reference-package <DIR> --expected-sha256 <SHA256> [--named-registry-sha256 <SHA256>] [--lineage-registry-sha256 <SHA256>]
cargo run --features dev --bin pterobacter-dev -- saved-evidence <EVIDENCE> --reference-package <DIR> --expected-sha256 <SHA256> --exclude-references <MASK.txt> [--report-dir <NEW_DIRECTORY>]
cargo run --features dev --bin pterobacter-dev -- rescue-plan <EVIDENCE> --reference-package <DIR> --expected-sha256 <SHA256> [--anim-records <RECORDS> --anim-method-profile <PROFILE>]
cargo run --features dev --bin pterobacter-dev -- skani-search --database <DIR>/databases/primary_whole --query <QUERY.fa> [--genomovar]
cargo run --features dev --bin pterobacter-dev -- genomovar-diagnostic --report <REPORT.json> --expected-report-sha256 <SHA256> --reference-package <DIR> --expected-sha256 <SHA256> [--exclude-evidence-row <REFERENCE_ID>]... [--remove-group <GENOMOVAR_ID>]...
cargo run --features dev --bin pterobacter-dev -- genomovar-diagnostic --report <REPORT.json> --expected-report-sha256 <SHA256> --reference-package <DIR> --expected-sha256 <SHA256> --setting self_alias --exclude-references <PRIMARY_MASK.txt> --exclude-references-sha256 <SHA256> --mask-member <REFERENCE_ID>
```

`saved-evidence` reads recorded skani comparisons and prints the species/subspecies decision as JSON, including raw nearest evidence, competitors, scope assessments and all withholding reasons. It runs no new comparisons. Use `pterobacter-dev saved-evidence --help` for the replay options. For classification from an assembly, see the [assembly guide](assembly.md).

`reference-package` authenticates a reference package directory against `--expected-sha256`, the SHA-256 that its `portable.json` must have. A manifest with any other digest is refused before its listing is read. Every file listed in the manifest must be present with the listed length and SHA-256, and no listed path may be a symbolic link. A file in the directory that the manifest does not list is refused. The package must carry a licence manifest with the NCBI submitter-rights disclaimer. `--named-registry-sha256` and `--lineage-registry-sha256` pin the two registry tables to digests you give. Nothing is pinned by default, so a table is checked against a digest only when you supply one.

The source manifest must be exactly the record the package's eleven tables, recipe and skani indexes derive, following the research package builder. The record covers concepts with their parents, references, genomovar definitions, the historical type crosswalk, context records, the cohort summary and every count. Each reference is identified by component and accession, so an accession can be both a primary reference and a genomovar definition member. The reporting cohort is a separate record set. Its membership and evidence flags must agree with the package, and its coverage categories are summarised as written. On success the command prints the package identity, licence, counts, cohort summary and components as JSON, including the SHA-256 of each database.

A package converted from the version 1 format carries the version 1 `portable.json` as an attested conversion record. The verifier checks that the record lists the same package, source manifest, export attestation and assets as the version 2 manifest.

`--reference-package` authenticates the package in the same way, then requires the evidence to be bound to it. The evidence's reference package digest must equal the verified manifest digest, or the digest of the attested version 1 manifest it was converted from. The evidence must cover the whole primary component. Each row must give what the package gives for its reference: the species and subspecies with their display names, the name status of the more specific of the two, the role (`type_anchor` or `diversity`), and no lineage. Contradictory evidence is refused, never corrected. The two flags must be given together. For rescue planning, saved ANIm decisions and reference masks, the verified package also supplies the primary panel in place of `--reference-inventory`; give one or the other. Without `--reference-package` the decision reports `reference_package.status` as `not_verified`.

`genomovar-diagnostic` accepts a completed assembly report with a completed, package-bound genomovar search. It checks the exact report bytes against `--expected-report-sha256`, verifies the package, and reuses saved evidence without running a new search. Repeat `--mask-member` to exclude definition members, `--remove-group` to omit whole groups, or `--exclude-evidence-row` to drop saved comparisons while retaining their required membership. The source report stays pinned and unchanged. Its `genomovar_id` remains null; `diagnostic_would_assign_genomovar_id` records the diagnostic candidate.

Member masks require `--setting self_alias` or `--setting clone_block` and a matching primary mask supplied through `--exclude-references` and `--exclude-references-sha256`. The parent is recomputed from the report's original primary evidence under that exact mask, bound to the same query, package and analysis settings. An additional warning envelope can be supplied through `--problem-warnings` and `--problem-warnings-sha256`; its artifacts are checked before recomputing the parent. An ANIm-enabled parent can also use `--anim-records` with `--anim-method-profile`, bound only to pairs requested by its masked plan. Valid parent withholding produces `diagnostic_outcome: gv_withheld_parent` and the parent reasons. Invalid evidence bindings produce an input error. The command exists only in the feature-gated development binary.

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
cargo run --features dev --bin pterobacter-dev -- anim-pair --query QUERY.fa --reference REFERENCE.fa --output NEW_DIRECTORY
```

Frozen evaluation contracts, command bindings and scorer mappings are retained under `docs/contracts/` for implementation audits.

## Layout

| Path | Role |
|---|---|
| `src/evidence.rs`, `src/annotations.rs`, `src/reference/`, `src/decision.rs`, `src/report.rs`, `src/mlst/` | Pure decision and typing logic. Package bytes arrive through a port, so there is no file, terminal or process code. |
| `src/cli.rs`, `src/cli/assembly.rs`, `src/cli/mlst.rs`, `src/dev.rs` | Argument parsing, files and exit codes for the public and development commands. |
| `src/bin/` | Thin entry points for `ptero`, `pterobacter` and `pterobacter-dev`. |
| `tests/` | Tests at the decision, command-line and packaging seams, with fixtures. |
| `docs/adr/` | Decisions and their reasons. |
| `CONTEXT.md` | Vocabulary. |

See [AGENTS.md](../AGENTS.md) for contribution rules and [CONTEXT.md](../CONTEXT.md) for the terms used here.
