# Whole-assembly classification

`tero` and `terobacter` accept one or more isolate assembly FASTAs, without a subcommand:

```sh
tero --reference-package reference --expected-sha256 MANIFEST_SHA256 isolate.fa > isolate.json
terobacter --reference-package reference --expected-sha256 MANIFEST_SHA256 --threads 4 first.fa second.fa > batch.json
```

Use the SHA-256 of `portable.json` supplied through a trusted package handoff. Computing a digest from an untrusted package does not establish its origin. The command verifies every listed asset, package metadata and component membership before opening the embedded skani database, and checks the package again before writing results. A provisioned run needs no network or external executable. Archived search labels are provenance labels; no archived FASTA is opened.

The default `--policy conservative-assembly-v1` uses the frozen species ANI ≥96%, subspecies ANI ≥98%, both AF ≥80% and subspecies margin ≥1 percentage point rules. The candidate `--policy conservative-assembly-anim-v2` runs the accepted skani-first rescue plan with embedded MUMmer4. Genomovar execution remains separate work. Historical MLST is optional under either policy. No query CheckM2 is run.

## Input and sequence scope

`--input-kind assembly` is the supported kind. FASTQ, empty input, empty records, missing identifiers, malformed FASTA and non-IUPAC nucleotide sequences fail. Needletail's supported compressed FASTA formats are accepted. Record headers must be UTF-8; duplicate headers remain separate records identified by ordinal.

All supplied records are passed to skani without plasmid filtering or trimming. Its pinned profile ignores records shorter than 500 bases when constructing sketches. `input.total_bases` and `input.records` include those records; `measurement.sketched_bases` shows the engine denominator. Eligibility by length does not guarantee informative sketch anchors. Sequence hashes cover each parser-emitted sequence as supplied, with case preserved and line breaks removed. `input.query_sha256` covers the exact file bytes, including compression when present. The engine measures the byte snapshot that was hashed and validated. The original query path string, recorded in `input.file`, also reaches skani: its upstream orientation tie rule uses that label. Preserve query and reference labels when replaying native evidence.

## Optional historical MLST

MLST runs only on request: pass a historical scheme package directory and the SHA-256 of its `manifest.json`, from a trusted source, together.

```sh
tero --reference-package reference --expected-sha256 MANIFEST_SHA256 \
  --mlst-package mlst --mlst-expected-sha256 MLST_MANIFEST_SHA256 isolate.fa > isolate.json
```

The engine is `terobacter-ungapped-mlst`, an in-binary ungapped seed-and-extend matcher that reproduces the calls of `mlst` 2.32.2 with BLAST+ 2.16.0 on the development fixtures. It is not that tool, and the report names the engine that ran. [MLST-UNGAPPED-r1](contracts/MLST-UNGAPPED-r1.md) describes the engine, the package checks and the record.

The scheme covers PubMLST submissions through 31 December 2024. It is a later retrieval, not a byte snapshot of that date, and it makes no claim to post-2024 currency. Every record carries the cutoff, retrieval time, package and data SHA-256s, data terms URL and limitation text. States are `exact_profile_match`, `unmatched_profile`, `incomplete_profile`, `inexact_alleles` and `ambiguous_alleles`. An exact profile the scheme lacks is unmatched; no new ST is claimed. A profile-compatible candidate ST for a repeated identical allele is never the reported ST.

MLST is supplementary. It never changes `assignment`, `species` or `subspecies`, and it types the same validated whole-input bytes as classification. Locations use each record's first header word, so repeated first words fail MLST for that input. Without the MLST options, reports carry `"mlst": {"requested": false, "status": "not_requested"}` and no MLST package is needed. A requested MLST that fails reports `mlst.status: failed` with its stage and error and exits 6. The classification result is kept.

## Reports and failures

One input produces a JSON object. Multiple inputs produce an array in argument order, with isolated statuses. They run sequentially, using `--threads` for each search. A failed input does not erase a completed input. The process exits nonzero if any input fails. Redirect stdout to retain the report; an unwritable output stream returns 1.

A completed search may produce an unresolved biological call, including when no estimate is returned. This is `execution.status: complete` with a null assignment. Execution failure instead reports `execution.status: incomplete`, the failed stage and an error, with null assignment fields. Partial reports retain validated input metadata where available. A failed final package check invalidates all decisions in that batch while retaining their measurement evidence.

Reports include authenticated data identity and version, policy ID, implementation version, skani revision and compatibility profile, floor 15%/screen 80% settings, every returned row and every unreturned reference. `evidence` is the package-bound decision input, retaining exact emitted decimal measurements. Scope is reported as not assessed when no scope assessment was performed; AF alone does not establish purity or explain extra sequence.

| Public exit code | Meaning |
|---|---|
| 0 | All inputs completed, whether assigned or biologically withheld. |
| 1 | Measurement, decision or output I/O failure. |
| 2 | Command-line usage error, including missing package/digest, invalid digest or zero threads. No report is available. |
| 3 | Unreadable, empty, malformed or unsupported assembly input or input kind. |
| 4 | Package authentication, database opening or final package recheck failed. |
| 5 | Requested native policy is unavailable, or ANIm flags are supplied to v1. No v1 fallback occurs. |
| 6 | MLST was requested and did not complete for an input whose classification completed, or its package changed during the run. Classification results are retained. |

For batches with different failures, the last failed input's code is returned; a final package recheck failure takes precedence. Inspect each report's execution status. Development commands retain their separate exit-code meanings in the README.

These are implementation and development-parity checks. Independent biological validation and release/platform acceptance remain separate gates.

## Candidate v2 ANIm rescue

```sh
tero --reference-package reference --expected-sha256 PRIMARY_MANIFEST_SHA256 \
  --policy conservative-assembly-anim-v2 \
  --anim-reference-package reference-fastas --expected-anim-sha256 ANIM_MANIFEST_SHA256 \
  --anim-output new-evidence isolate.fa > isolate.json
```

V2 uses the same whole-input skani search. Outside the rescue band it keeps the v1 decision and requires no ANIm component. Eligible queries measure witnesses first. A passing witness requires the entire primary competitor panel, including unreturned references. A decisive veto permits an early stop; the report lists every omitted reference. A positive rescue assigns species only, with null subspecies and genomovar. The report retains `legacy_v1_result`, the original skani evidence, the final plan, exact reciprocal records, artifact directories and hashes. Validation status remains `not_yet_validated`.

Missing required FASTAs, invalid metadata or bytes, failed native execution and interruption produce an incomplete report with a null call and nonzero exit. Successful zero alignment is measured evidence and counts separately from failures. Redirect stdout to save the unified report. `--anim-output` must be a new directory; each query and pair receives a separate retained directory. There is no cache reuse in this slice.

ANIm runs one worker at a time with one native query thread. Defaults are 120 seconds per reciprocal pair, 600 seconds total ANIm wall time per query and 1024 MiB per worker. `--anim-pair-seconds`, `--anim-query-seconds` and `--anim-memory-mib` select limits up to 1800 seconds, 7200 seconds and 6144 MiB respectively; minimum memory is 64 MiB. Time limits accept positive fractional seconds down to one nanosecond. SIGINT and SIGTERM stop the native worker and retain partial evidence. A complete execution stopped by a biological veto can still report `competitor_evaluation: incomplete`, because the pure decision records which competitors were not measured; `anim_execution.stop_reason: decisive_veto` distinguishes this valid stop from a failed required comparison. The public command never silently substitutes v1 for requested v2.

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

The maintained MUMmer4 profile preserves literal N identity and inclusive whole-input coverage, matching the accepted MUMmer3 comparator semantics. Long runs of N remain a known limitation. See the [method amendment](contracts/P1-ANIM-MUMMER4-r1.md). Duplicate FASTA identifiers accepted by v1 cannot be passed to the native ANIm adapter; if rescue requires such an input, v2 reports an explicit failed pair.
