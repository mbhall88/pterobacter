# Whole-assembly v1 classification

`tero` and `terobacter` accept one or more isolate assembly FASTAs, without a subcommand:

```sh
tero --reference-package reference --expected-sha256 MANIFEST_SHA256 isolate.fa > isolate.json
terobacter --reference-package reference --expected-sha256 MANIFEST_SHA256 --threads 4 first.fa second.fa > batch.json
```

Use the SHA-256 of `portable.json` supplied through a trusted package handoff. Computing a digest from an untrusted package does not establish its origin. The command verifies every listed asset, package metadata and component membership before opening the embedded skani database, and checks the package again before writing results. A provisioned run needs no network or external executable. Archived search labels are provenance labels; no archived FASTA is opened.

The default `--policy conservative-assembly-v1` uses the frozen species ANI ≥96%, subspecies ANI ≥98%, both AF ≥80% and subspecies margin ≥1 percentage point rules. Native v2 is unavailable; `--policy conservative-assembly-anim-v2` produces an incomplete report and exits 5. Saved-evidence v2 remains in the development binary. ANIm and genomovar execution are outside this command's current v1 scope. No query CheckM2 is run.

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
| 5 | Requested native policy is unavailable. No v1 fallback occurs. |
| 6 | MLST was requested and did not complete for an input whose classification completed, or its package changed during the run. Classification results are retained. |

For batches with different failures, the last failed input's code is returned; a final package recheck failure takes precedence. Inspect each report's execution status. Development commands retain their separate exit-code meanings in the README.

These are implementation and development-parity checks. Independent biological validation and release/platform acceptance remain separate gates.
