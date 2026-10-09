# Whole-assembly v1 classification

`tero` and `terobacter` accept one or more isolate assembly FASTAs, without a subcommand:

```sh
tero --reference-package reference --expected-sha256 MANIFEST_SHA256 isolate.fa > isolate.json
terobacter --reference-package reference --expected-sha256 MANIFEST_SHA256 --threads 4 first.fa second.fa > batch.json
```

Use the SHA-256 of `portable.json` supplied through a trusted package handoff. Computing a digest from an untrusted package does not establish its origin. The command verifies every listed asset, package metadata and component membership before opening the embedded skani database, and checks the package again before writing results. A provisioned run needs no network or external executable. Archived search labels are provenance labels; no archived FASTA is opened.

The default `--policy conservative-assembly-v1` uses the frozen species ANI ≥96%, subspecies ANI ≥98%, both AF ≥80% and subspecies margin ≥1 percentage point rules. Native v2 is unavailable; `--policy conservative-assembly-anim-v2` produces an incomplete report and exits 5. Saved-evidence v2 remains in the development binary. ANIm, MLST and genomovar execution are outside this command's current v1 scope. No query CheckM2 is run.

## Input and sequence scope

`--input-kind assembly` is the supported kind. FASTQ, empty input, empty records, missing identifiers, malformed FASTA and non-IUPAC nucleotide sequences fail. Needletail's supported compressed FASTA formats are accepted. Record headers must be UTF-8; duplicate headers remain separate records identified by ordinal.

All supplied records are passed to skani without plasmid filtering or trimming. Its pinned profile ignores records shorter than 500 bases when constructing sketches. `input.total_bases` and `input.records` include those records; `measurement.sketched_bases` shows the engine denominator. Eligibility by length does not guarantee informative sketch anchors. Sequence hashes cover each parser-emitted sequence as supplied, with case preserved and line breaks removed. `input.query_sha256` covers the exact file bytes, including compression when present. The engine measures the byte snapshot that was hashed and validated. The original query path string, recorded in `input.file`, also reaches skani: its upstream orientation tie rule uses that label. Preserve query and reference labels when replaying native evidence.

## Reports and failures

One input produces a JSON object. Multiple inputs produce an array in argument order, with isolated statuses. They run sequentially, using `--threads` for each search. A failed input does not erase a completed input. The process exits nonzero if any input fails. Redirect stdout to retain the report; an unwritable output stream returns 1.

A completed search may produce an unresolved biological call, including when no estimate is returned. This is `execution.status: complete` with a null assignment. Execution failure instead reports `execution.status: incomplete`, the failed stage and an error, with null assignment fields. Partial reports retain validated input metadata where available. A failed final package check invalidates all decisions in that batch while retaining their measurement evidence.

Reports include authenticated data identity and version, policy ID and gates, implementation version, skani revision and compatibility profile, floor 15%/screen 80% settings, every returned row and every unreturned reference. `evidence` is the package-bound decision input, retaining exact emitted decimal measurements. Scope is reported as not assessed when no scope assessment was performed; AF alone does not establish purity or explain extra sequence.

| Public exit code | Meaning |
|---|---|
| 0 | All inputs completed, whether assigned or biologically withheld. |
| 1 | Measurement, decision or output I/O failure. |
| 2 | Command-line usage error, including missing package/digest, invalid digest or zero threads. No report is available. |
| 3 | Unreadable, empty, malformed or unsupported assembly input or input kind. |
| 4 | Package authentication, database opening or final package recheck failed. |
| 5 | Requested native policy is unavailable. No v1 fallback occurs. |

For batches with different failures, the last failed input's code is returned; a final package recheck failure takes precedence. Inspect each report's execution status. Development commands retain their separate exit-code meanings in the README.

These are implementation and development-parity checks. Independent biological validation and release/platform acceptance remain separate gates.
