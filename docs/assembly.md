# Whole-assembly classification

`ptero` and `pterobacter` accept one or more isolate assembly FASTAs, without a subcommand:

```sh
ptero --reference-package reference --expected-sha256 MANIFEST_SHA256 \
  --anim-reference-package reference-fastas --expected-anim-sha256 ANIM_MANIFEST_SHA256 \
  --anim-output isolate-alignments isolate.fa > isolate.json
pterobacter --no-anim --reference-package reference --expected-sha256 MANIFEST_SHA256 \
  --jobs 2 --threads 4 --output batch.json first.fa second.fa
```

Use the SHA-256 of `portable.json` supplied through a trusted package handoff. Computing a digest from an untrusted package does not establish its origin. The command verifies every listed asset, package metadata and component membership before opening the embedded skani database, and checks the package again before writing results. A provisioned run needs no network or external executable: no dependency is a network client, the source opens no socket, and where unprivileged namespaces are available, the test suite runs skani with MLST and a boundary ANIm analysis inside a network namespace that has no interface but loopback. Archived search labels are provenance labels; no archived FASTA is opened.

## ANIm near the species boundary

Pterobacter first estimates ANI with skani. For eligible samples just below the species boundary, it also runs ANIm using whole-genome alignments to refine the estimate. ANIm can take longer, especially in large batches with many borderline samples; samples outside that band do not incur alignment work. Use `-n` or `--no-anim` to skip ANIm and use skani only.

The standard thresholds are skani ANI ≥96% for species and ≥98% for subspecies, both aligned fractions ≥80%, and a subspecies margin ≥1 percentage point. ANIm is considered when 94.5% ≤ maximum skani ANI <96%, subject to coverage and other safeguards. An ANIm-supported assignment requires ANI strictly greater than 96% in both directions and completed required competitor comparisons. It assigns species only; subspecies and genomovar stay unset.

Supply the [matching reference FASTAs and evidence directory](#boundary-anim). Database-managed setup is planned in [issue39](https://github.com/mbhall88/terobacter/issues/39); these explicit paths and digests are still required by the current build. MLST remains optional. [Genomovar reporting](genomovar.md) depends on an eligible parent call. No query CheckM2 is run.

`--no-anim` cannot be combined with custom ANI thresholds or alignment-specific options. It avoids MUMmer4 at runtime; building still requires the embedded aligner's C++ dependencies. See [all options](cli-help.md).

## Input and sequence scope

Supply isolate assembly FASTAs. FASTQ, empty input, empty records, missing identifiers, malformed FASTA and non-IUPAC nucleotide sequences fail. Needletail's supported compressed FASTA formats are accepted. Record headers must be UTF-8; duplicate headers remain separate records identified by ordinal.

All supplied records are passed to skani without plasmid filtering or trimming. Its pinned profile ignores records shorter than 500 bases when constructing sketches. `input.total_bases` and `input.records` include those records; `measurement.sketched_bases` shows the engine denominator. Eligibility by length does not guarantee informative sketch anchors. Sequence hashes cover each parser-emitted sequence as supplied, with case preserved and line breaks removed. `input.query_sha256` covers the exact file bytes, including compression when present. The engine measures the byte snapshot that was hashed and validated. The original query path string, recorded in `input.file`, also reaches skani: its upstream orientation tie rule uses that label. Preserve query and reference labels when replaying native evidence.

## Optional historical MLST

MLST runs only on request: pass a historical scheme package directory and the SHA-256 of its `manifest.json`, from a trusted source, together.

```sh
ptero --reference-package reference --expected-sha256 MANIFEST_SHA256 \
  --anim-reference-package reference-fastas --expected-anim-sha256 ANIM_MANIFEST_SHA256 \
  --anim-output typed-alignments \
  --mlst-package mlst --mlst-expected-sha256 MLST_MANIFEST_SHA256 isolate.fa > isolate.json
```

The engine is `terobacter-ungapped-mlst`, an in-binary ungapped seed-and-extend matcher that reproduces the calls of `mlst` 2.32.2 with BLAST+ 2.16.0 on the development fixtures. It is not that tool, and the report names the engine that ran. [MLST-UNGAPPED-r1](contracts/MLST-UNGAPPED-r1.md) describes the engine, the package checks and the record.

The scheme covers PubMLST submissions through 31 December 2024. It is a later retrieval, not a byte snapshot of that date, and it makes no claim to post-2024 currency. Every record carries the cutoff, retrieval time, package and data SHA-256s, data terms URL and limitation text. States are `exact_profile_match`, `unmatched_profile`, `incomplete_profile`, `inexact_alleles` and `ambiguous_alleles`. An exact profile the scheme lacks is unmatched; no new ST is claimed. A profile-compatible candidate ST for a repeated identical allele is never the reported ST.

MLST is supplementary. It never changes `assignment`, `species` or `subspecies`, and it types the same validated whole-input bytes as classification. Locations use each record's first header word, so repeated first words fail MLST for that input. Without the MLST options, reports carry `"mlst": {"requested": false, "status": "not_requested"}` and no MLST package is needed. A requested MLST that fails reports `mlst.status: failed` with its stage and error and exits 6. The classification result is kept.

## Reports and failures

One input produces a JSON object. Multiple inputs produce an array in argument order, with isolated statuses. Each report records its argument position as `input.ordinal` beside `input.file`. A failed input does not erase or change a completed input. The process exits nonzero if any input fails.

`--report-dir NEW_DIRECTORY` also writes `report.json` (the same JSON as the main report), `calls.tsv` (one row per assembly), `evidence.tsv` (strongest matches, whole-panel parents and challenges) and `report.md`. The directory must not exist; a collision exits 2 before any analysis, and no file is replaced. The TSV and Markdown are derived from the JSON and take a label only from its final call. See the [usage guide](usage.md).

`--jobs N` analyses up to N inputs at once (default 1). Each runs its searches with `--threads`, so a run uses up to N × `--threads` search threads, and up to N native ANIm workers when needed, each limited by `--anim-memory-mib`. Calls, statuses, exit codes and evidence directory names do not depend on N or on the order in which inputs finish; timings and, with a shared cache, which query stored or reused a pair can.

The report goes to stdout unless `--output FILE` names a new file in an existing directory. An existing file is never replaced: the run is refused before any work, and a file that appears during the run is left alone, in which case the report goes to stdout and the run exits 1. The file is written completely before it appears, so a reader never sees a partial report. An unwritable stdout returns 1.

Output paths are checked before any work, and a collision exits 2 without a report. The report file, `--report-dir`, `--anim-output` and `--anim-cache` must be distinct and not nested in one another. None may be inside the reference package, the ANIm component or the MLST package, and none may be an input assembly. Paths are compared after resolving symbolic links and `..`.

A completed search may produce an unresolved biological call, including when no estimate is returned. This is `execution.status: complete` with a null assignment. Execution failure instead reports `execution.status: incomplete`, the failed stage and an error, with null assignment fields. Partial reports retain validated input metadata where available. A missing or corrupt reference package or database fails each input that reaches it with exit 4, keeping the validated input metadata. A missing or invalid ANIm component fails a query that needs alignments at the `anim` stage with exit 1, keeping its plan and any pair evidence; one that changes after its pairs complete fails the final component check with exit 4. A package changed during the run fails the final check, which invalidates all decisions in that batch while retaining their measurement evidence.

The first SIGINT or SIGTERM stops the run at the next check with or without ANIm. Inputs not yet started report `run interrupted`. A running skani search finishes, and a native ANIm worker is stopped. Every call in the batch is then withheld at the `publication` stage, measurement and ANIm evidence are kept, the report is still written and the exit code is 1. A second signal of the same kind ends the process at once without a report.

Reports include authenticated data identity and version, analysis settings, implementation version, skani revision and compatibility profile, floor 15%/screen 80% settings, every returned row and every unreturned reference. `evidence` is the package-bound decision input, retaining exact emitted decimal measurements. Scope is reported as not assessed when no scope assessment was performed; AF alone does not establish purity or explain extra sequence.

| Public exit code | Meaning |
|---|---|
| 0 | All inputs completed, whether assigned or biologically withheld. |
| 1 | Measurement, decision, genomovar search or output I/O failure, or the run was interrupted. |
| 2 | Command-line usage error, including missing package/digest, invalid digest, zero threads or jobs, an existing `--output` file, a missing directory for it or an output collision. No report is available. |
| 3 | Unreadable, empty, malformed or unsupported assembly input or input kind. |
| 4 | Package authentication, database opening or final package recheck failed. |
| 5 | Alignment-specific options were supplied with `--no-anim`. |
| 6 | MLST was requested and did not complete for an input whose classification completed, or its package changed during the run. Classification results are retained. |

For batches with different failures, the last failed input's code in argument order is returned. A final package recheck failure takes precedence over it, and an interruption over both. Inspect each report's execution status. Development commands retain their separate [exit-code meanings](development.md#development-exit-codes).

These are implementation and development-parity checks. Independent biological validation and release/platform acceptance remain separate gates.

## Boundary ANIm

```sh
ptero --reference-package reference --expected-sha256 PRIMARY_MANIFEST_SHA256 \
  --anim-reference-package reference-fastas --expected-anim-sha256 ANIM_MANIFEST_SHA256 \
  --anim-output new-evidence isolate.fa > isolate.json
```

Analysis starts with the whole-input skani search. Outside the boundary band it keeps the skani decision and requires no ANIm component. Eligible queries measure witnesses first. A passing witness requires the entire primary competitor panel, including unreturned references. A decisive veto permits an early stop; the report lists every omitted reference. A positive rescue assigns species only, with null subspecies and genomovar. The report retains the initial skani result and evidence, the final plan, exact reciprocal records, artifact directories and hashes. Validation status remains `not_yet_validated`.

Missing required FASTAs, invalid metadata or bytes, failed native execution and interruption produce an incomplete report with a null call and nonzero exit. Successful zero alignment is measured evidence and counts separately from failures. Redirect stdout to save the unified report. `--anim-output` must be a new directory; each query and pair receives a separate retained directory, named by the query's argument position.

Within each query, ANIm runs one worker at a time with one native query thread. Defaults are 120 seconds per reciprocal pair, 600 seconds total ANIm wall time per query and 1024 MiB per worker. `--anim-pair-seconds`, `--anim-query-seconds` and `--anim-memory-mib` select limits up to 1800 seconds, 7200 seconds and 6144 MiB respectively; minimum memory is 64 MiB. Time limits accept positive fractional seconds down to one nanosecond. SIGINT and SIGTERM stop the native worker and retain partial evidence. A complete execution stopped by a biological veto can still report `competitor_evaluation: incomplete`, because the pure decision records which competitors were not measured; `anim_execution.stop_reason: decisive_veto` distinguishes this valid stop from a failed required comparison. A failed required alignment never silently falls back to a skani-only call.

## Reusing reciprocal pairs

`--anim-cache DIR` keeps verified reciprocal pairs for later queries and runs. The directory is created when absent and may be shared by runs. A pair is reused only when every part of its key matches:

- the query's exact file bytes, its component and its whole-assembly scope;
- the reference ID, exact FASTA bytes, component, concept IDs, scope and the primary package the component is bound to;
- the method profile, amendment, native build and source hashes, compiler, target and Pterobacter version;
- the pair time limit and worker memory limit, since a pair that finished under a looser limit could fail under a tighter one.

A reused pair also counts the native time recorded in its receipts against the query's `--anim-query-seconds` budget, and is measured again instead when that time exceeds what remains for the pair. The cache therefore never lets a query complete that could not complete without it.

The same bytes under another file name are the same query. Before reuse, the saved key must equal the requested key, and the saved pair is verified again as any saved record is: snapshot bytes, native artifacts, roles, receipts and integer metrics. The method and limits in the record must also agree with the key. A reused pair is copied into the query's `--anim-output` directory and verified there. Its record keeps the receipts of the run that measured it.

Each pair reports `cache.status` and `cache.key_sha256`. The statuses are `reused`; `stored`, for a new verified pair saved for later; `already_stored`, when a concurrent query saved it first; and `rejected`, when an entry at that key fails a check or needs more time than remains. A rejected entry is measured again and left unchanged for inspection; delete it to let the cache store that pair again. `not_stored` means the pair failed and was not saved. `store_failed` means saving failed; the measurement still counts. Entries appear whole or not at all and are never replaced. The cache is local working storage: its keys and checks bind entries to comparisons, but they do not authenticate the directory against a writer who can forge a consistent entry.

## Separate reference FASTA component

The optional component has its own directory and trusted SHA-256 for `anim.json`. It contains every primary whole-assembly source FASTA and an exact copy of the primary package's licence manifest. FASTA bytes, lengths, IDs and concept context must match the authenticated primary package. Chromosome-only substitutes, incomplete inventories, extra files, unsafe paths and symbolic links are refused. The component is verified before alignments and again before completing the native run; the primary package is also rechecked before publishing the batch.

Its manifest has this shape; reference entries repeat for the complete primary panel:

```json
{
  "schema": "terobacter.anim-reference/1",
  "component_id": "primary_whole",
  "sequence_scope": "whole_assembly_as_deposited",
  "reference_package_sha256": "PRIMARY_MANIFEST_SHA256",
  "licence": {"path": "licence.json", "sha256": "LICENCE_SHA256", "bytes": 123},
  "references": [{
    "reference_id": "primary_whole:ACCESSION",
    "path": "sequences/ACCESSION.fa",
    "sha256": "SOURCE_SHA256",
    "bytes": 456,
    "concept_ids": ["SPECIES_ID", "SUBSPECIES_ID"]
  }]
}
```

Concept IDs are ordered species then subspecies, omitting the latter when absent. Paths are relative to the component directory. Each FASTA is capped at 256 MiB and the manifest at 1 MiB. Keep this component separate from the skani package, whose inventory rejects unlisted files. Obtain both trusted digests from the package handoff. Component provisioning and distribution remain release work.

Native pair snapshots retain the exact supplied bytes, including compression, in `inputs/query.fa`. Each pair's `query.sha256` and the report's `input.query_sha256` identify those original bytes. The adapter uses the same compressed-format decoder as the assembly reader, then validates and normalizes FASTA formatting for `inputs/query.native.fa`; `query.native_sha256` identifies the bytes read by both native orientations. Verification decodes the retained original snapshot again and checks the native bytes, record lengths and hashes. All records, sequence case and ambiguous bases are preserved. The 256 MiB native-input limit also applies to decoded FASTA.

The maintained MUMmer4 profile preserves literal N identity and inclusive whole-input coverage, matching the accepted MUMmer3 comparator semantics. Long runs of N remain a known limitation. See the [method amendment](contracts/P1-ANIM-MUMMER4-r1.md). Duplicate FASTA identifiers accepted by skani cannot be passed to the native ANIm adapter; if rescue requires such an input, the report records an explicit failed pair.
