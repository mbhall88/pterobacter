# ANIm feasibility: embedded MUMmer 3.23 or minimap2?

Issue [#1](https://github.com/mbhall88/terobacter/issues/1), 9 October 2026. Investigator: Codex, GPT-6 (the exact backend identifier is not exposed). This directory contains development probes and synthetic engineering measurements, not the production classifier.

**Recommend embedding MUMmer 3.23 to preserve P1.** The native-stage probe matched the comparator's integers and normalized filtered deltas on all **160 synthetic pairs / 320 orientations**. Minimap2 is faster on these workloads but changes measurements and threshold outcomes. Selecting minimap2 requires a new method profile and explicit scientific change control. Michael retains the route decision.

The preregistered [protocol](protocol.md) fixes fixtures, stop rules and the exact-integer equivalence criterion. [Diagnostic deviations](protocol-deviations.md) retain reader/API problems rather than hiding them. All inputs are generated synthetic DNA. No new biological evaluation, reference-package export, classifier implementation or scientific-policy amendment was performed.

## Results and equivalence

The comparator is P1 `reciprocal-nucmer3-mum-1-pyani030-span-v1`: MUMmer 3.23, `nucmer --mum`, one-to-one `delta-filter -1`, identity `100(D−2E)/D`, inclusive interval-union AFs. D is the sum of both block spans; E is the fifth delta-header integer. Each reciprocal pair retains two identities and four AFs mapped back to the same biological roles. The pinned pyANI 0.3.0a0 parser independently agrees on the 38 aligned matrix orientations. Its division by zero on the two valid empty deltas is retained as a legacy limitation; P1 requires explicit successful `no_alignment` instead.

Exact equivalence here means identical D/E, covered bases, whole lengths and statuses in both directions; no decimal tolerance or rounding is permitted. Normalized deltas exclude only the first filename line. Passing these development fixtures does not prove arbitrary-genome or cross-platform equivalence.

| Route | Exact matrix pairs / 20 | Pairs with an ANI/AF gate difference | Exact panel pairs / 140 | Classification |
|---|---:|---:|---:|---|
| Embedded MUMmer 3.23 | 20 | 0 | 140 | Equivalent on the bounded Linux fixtures; production/platform validation pending |
| minimap2 asm5, file-index API | 8 | 5 | 16 | Needs change control |
| minimap2 asm10, file-index API | 8 | 4 | 23 | Needs change control |
| minimap2 asm20, file-index API | 8 | 4 | 27 | Needs change control |
| Rust-Bio banded / WFA-style pairwise primitive | Not measured | Not measured | Not measured | Blocked as a bounded whole-genome P1 replacement; missing genome anchoring and one-to-one selection |

[All directional observations](results/observations.tsv), [all differences](results/differences.tsv), and [machine-readable summary](results/summary.json) retain every matrix row, including successful no-alignment. The tables use `NA` for unavailable values; the status distinguishes failed execution from successful no-alignment. The standalone AF tests confirm that overlaps count once and singleton/reverse intervals remain inclusive.

The five nominal 94.5–96% substitution cases are reported separately in [rescue-band differences](results/rescue-band-differences.tsv). These are nominal DNA mutation cases, not claims that measured skani activated a rescue. Highlights:

- At nominal 96%, MUMmer gives **96.000833%**, while asm10/asm20 give **96.000000%** in both orientations. The strict `>96` gate changes, despite identical 100% AFs.
- At nominal 94.5%, asm5 covers about **55.983%**, compared with MUMmer's 100%, and changes the AF80 gates. Its identity is about 94.720%, versus 94.501% for MUMmer.
- The exact shared-prefix AF79.9/80/80.1 fixtures have zero numerical AF difference across all measured profiles. The AF80 equality passes, while 79.9% fails. See [AF-boundary differences](results/af-boundary-differences.tsv). This narrow agreement does not establish coverage parity on other structures.
- On repeated sequence, minimap2 aligns across a 40-kb gap and produces about **71.429%** span identity, while MUMmer's selected local blocks give 100%. On block permutation, minimap2 gives 60% versus 100%. Inversions, indel tracts and substitutions also differ. The final file-index profile handles fragmentation in both directions; the earlier in-memory profile missed the reverse orientation.

All five measured profiles report valid independent random inputs as `no_alignment`, null identity and zero AF, with exit zero. Four invalid inputs (alphabet, duplicate identifiers, empty FASTA, missing file) are rejected per profile. The comparator is protected by the same development input guard; permissive native-tool handling is not presented as supported input validation. Missing-output-parent failures return nonzero for MUMmer; a full output device produces a nonzero Rust write failure for minimap2. Failures stay distinct from biological no-alignment. These are development error checks, not a finished production error interface.

## Resource measurements

One 5-Mb synthetic 4.5%-substitution pair, both orientations, on Linux x86_64 / AMD EPYC 9745. Each profile was timed once; these are observations, not a replicated hardware benchmark.

| Profile | Pair wall seconds | Pair CPU seconds | Largest orientation GNU peak RSS, MiB |
|---|---:|---:|---:|
| Pinned comparator | 15.14 | 14.77 | 158.06 |
| Embedded MUMmer | 13.46 | 13.30 | 83.04 |
| asm5, file index | 2.60 | 2.56 | 97.23 |
| asm10, file index | 2.27 | 2.22 | 90.99 |
| asm20, file index | 2.53 | 2.47 | 123.16 |

A complete 140-reference workload was measured with **500-kb synthetic references**, two orientations per reference, eight workers, no pruning. Input generation is excluded; alignment, orchestration and metric collection are included in panel wall time.

| Profile | Panel wall seconds | Sum CPU seconds | Sampled aggregate worker RSS, MiB |
|---|---:|---:|---:|
| Pinned comparator | 36.54 | 211.68 | 281.06 |
| Embedded MUMmer | 31.14 | 178.13 | 142.43 |
| asm5, file index | 17.71 | 86.55 | 173.47 |
| asm10, file index | 9.79 | 44.64 | 142.95 |
| asm20, file index | 10.18 | 47.77 | 159.02 |

Panel memory is a 50-ms sample of summed process-tree RSS, excluding the Python coordinator; shared mappings can be counted more than once and brief peaks can be missed. GNU time gives per-process/descendant maxima rather than simultaneous pipeline memory. It reported zero RSS on fast embedded stages; these cells are unavailable, with a separate 10-ms process-tree diagnostic in the observation table. That diagnostic gives about 11–12 MiB per small embedded orientation. Sampling and normal filesystem contention affect timing. Do not extrapolate the synthetic panel to laptop or clinical-genome performance.

For context, already exposed [P2 evidence](https://github.com/mbhall88/Enterobacter/issues/21) measured one development query against the actual 140-reference panel: 5,718.5 CPU seconds and 735.573 wall seconds with eight workers. That evidence was reused, not rerun. It is a different input/workload and does not measure this embedded implementation on the real panel.

Stripped release probes, compared with an empty Rust executable built with the same optimization/LTO/codegen settings:

| Binary | Total bytes | Added bytes above empty release |
|---|---:|---:|
| Empty release | 301,936 | 0 |
| Embedded MUMmer probe | 563,264 | 261,328 |
| minimap2 file-index probe | 576,360 | 274,424 |

These are standalone-probe increments, not the eventual Terobacter binary size. Native algorithms and zlib are linked into the probes; Linux system C/C++ libraries remain dynamic. Both candidates run with an empty executable PATH. Source audit shows embedded workers execute only the current binary. `strace` was unavailable because host ptrace is prohibited, so there is no syscall-trace claim.

## Implementation, licence and platform costs

The embedded probe rebuilds MUMmer's `prenuc`, `mummer`, `mgaps`, `postnuc` and `delta-filter` from the existing 3.23 source distribution. It preserves nucmer defaults: unique-in-both anchors, both strands, minimum match 20, cluster 65, maximum gap 90, diagonal difference 5, factor .12 and extension break 200. Linux GNU object tools localize conflicting stage symbols. Each stage runs in a fresh process of the same executable, containing legacy globals, getopt state and `exit()` without another runtime program. Intermediate FASTA/match/cluster/delta artifacts are files. This establishes a plausible self-contained route, not a reentrant or browser-ready library.

Production work still needs: reproducible vendoring/pins and build integration; portable symbol isolation; checked artifact/receipt/cache handling; process/resource/error/cancellation isolation; meaningful platform replay; pinned-parser conformance. Do not copy this prototype into production unchanged. Legacy entry points and global state make direct repeated calls in one process unsafe without additional work. No engine has been selected or policy renamed.

MUMmer's inspected 3.23 distribution has **The Artistic License** ([upstream source repository](https://github.com/marbl/MUMmer3)); retain its source/notice and modification/distribution requirements during release review. No upstream sources are exported here. The Rust binding declares **MIT OR Apache-2.0**, and bundled minimap2 and zlib are MIT and Zlib respectively; actual inspected licence text and hashes are retained with research provenance. Build and runtime dependency notices still need release review. Primary sources: [binding metadata](https://docs.rs/crate/minimap2/0.1.31+minimap2.2.30/source/Cargo.toml.orig), [minimap2 licence](https://github.com/lh3/minimap2/blob/v2.30/LICENSE.txt).

Linux release build, runtime matrix, invalid/failure checks and PATH-empty checks succeeded. Apple arm64 and x86_64 cross-check attempts failed in native zlib compilation because this host's C compiler cannot accept Apple's `-arch`/deployment-target flags. The MUMmer embedding build is deliberately Linux-only because it uses GNU ELF tools. **No macOS runtime or support claim** is made; actual Mac build/replay is a downstream gate. Native cross-build failures on this host do not establish that minimap2 fails on a Mac.

Browser obstacles: embedded MUMmer relies on process spawning and native filesystem artifacts, unavailable in this form in a browser. It would need a different worker/reentrant architecture and a separate parity gate. The current minimap2 binding uses Unix path access/native C builds, SIMD and zlib; browser threads, memory limits, file/index APIs and wasm compilation remain untested. Neither native Rust packaging nor an in-memory input method proves browser portability.

The short survey found [Rust-Bio's banded pairwise aligner](https://docs.rs/bio/latest/bio/alignment/pairwise/banded/struct.Aligner.html) and [WFA2](https://github.com/smarco/WFA2-lib) as alignment primitives. Neither documented interface supplies P1's whole-genome unique anchors, rearrangement segmentation and reciprocal one-to-one filtering as a replacement pipeline. Building those around a primitive exceeds this bounded ticket. No fabricated numerical, size or timing comparison is supplied for that unbuilt route.

## Change-control map and MLST handoff

Embedded MUMmer is the recommended compatibility candidate. Preserve the P1 semantics but bind the new embedded source/executable/parser hashes; prove production R3 integers before asserting equivalence. P4a-r3 remains untouched. Its EN-3 platform parity and EN-4 integer/parser parity gates still apply, as do R3 receipts and CO-2 cost reporting.

A minimap2 selection would require a versioned P1 amendment specifying engine/crate/C source pins, index API, preset/options, primary/supplementary selection, base-level error counting, span formula, interval-union coverage, whole-input scopes and no-alignment/error states. Map changed numerical witnesses, strict identity boundaries, AF boundaries, reciprocal consistency and foreign-parent vetoes to P4a-r3's R3/EN-4/RS-1/CO-2 and platform gates. Record a corresponding evaluation amendment before Enterobacter #25; do not edit the accepted P4a-r3 in place or relabel minimap2 as the pinned nucmer profile. These fixtures quantify engineering discrepancies, not effects on frozen biological accuracy endpoints.

For [MLST feasibility #2](https://github.com/mbhall88/terobacter/issues/2):

- Pin `minimap2 0.1.31+minimap2.2.30` / `minimap2-sys 0.1.30+minimap2.2.30` and the lockfile. Static-link via the sys crate; the high-level static feature activates optional rust-htslib and its build dependencies.
- Use numerical CIGAR/NM and explicit coordinates; `cigar_str` adds terminal soft clips even with the binding's clipping flag disabled. Approximate `dv` and gap-compressed `de` are not this identity. The [minimap2 manual](https://github.com/lh3/minimap2/blob/v2.30/minimap2.1) describes these tags and presets.
- Verify primary/supplementary filtering in the returned mapping records, not just the CLI-style output flag. No-secondary filtering here keeps supplementary alignments.
- Test the intended index API with multiple contigs/loci. The in-memory and file APIs were observably different on the fragmented fixture; the former also hardcodes `mid_occ=1000`. No root-cause or upstream-bug claim is made. File API results, not the earlier diagnostic pass, appear in the tables above.
- The genome-alignment presets measured here are not an MLST allele-matching recommendation; short-allele settings and historical partial/inexact states require that ticket's own bounded fixture comparison.

## Reproduce the development measurements

Prerequisites: Rust/Cargo, Python 3, GCC/G++, GNU nm/objcopy/ld/ar and GNU time on Linux. For the original embedded probe, supply an isolated MUMmer **3.23** source copy; source provenance is part of the caller's research receipt. The separately authorized [MUMmer4 continuation](mummer4.md) has its own probe, frozen addendum and results. Probes are separate, unpublished development packages, independent of the production crate ticket.

```bash
python -m unittest discover -s investigations/anim -p 'test_*.py'
MUMMER3_SOURCE=/absolute/path/to/MUMmer3 \
  cargo build --locked --release --features embedded-mummer3 \
  --manifest-path investigations/anim/probes/Cargo.toml
MUMMER3_SOURCE=/absolute/path/to/MUMmer3 \
  cargo clippy --locked --all-targets --features embedded-mummer3 \
  --manifest-path investigations/anim/probes/Cargo.toml -- -D warnings
python investigations/anim/generate_fixtures.py /absolute/path/to/new-fixtures
python investigations/anim/measure.py \
  --fixtures /absolute/path/to/new-fixtures \
  --output /absolute/path/to/new-measurements \
  --nucmer /absolute/path/to/pinned/nucmer \
  --delta-filter /absolute/path/to/pinned/delta-filter \
  --mummer-probe investigations/anim/probes/target/release/mummer3-probe \
  --minimap-probe investigations/anim/probes/target/release/minimap2-probe \
  --budget-ledger /absolute/path/to/same-experiment/budget.json \
  --panel --sample-pairs
python investigations/anim/summarize.py \
  --first /absolute/path/to/new-measurements \
  --fileapi /absolute/path/to/new-measurements \
  --memorydiag /absolute/path/to/new-measurements \
  --output /absolute/path/to/new-summary
```

Validation: 11 metric/harness tests pass, including error-row publication and termination of timed-out descendants; Clippy with warnings denied, Rust formatting and Git whitespace checks pass. The code review found two harness defects, both corrected before handoff.

The existing research run retains raw deltas/PAF, both binding API passes, command arrays, input/source/executable hashes, timing/RSS records, build logs, frozen-context verification, failure diagnostics and manuscript contribution. Those receipts contain local research paths and stay out of this repository. This repo contains synthetic observations and reusable development probe sources only. The handoff on issue #1 locates the private research receipt. Scientific acceptance, macOS execution and the production implementation remain separate review steps.

The continuation corrects review R1 without replacing those measurements: a single reciprocal-pair deadline and persistent conservative admission ledger cover matrix, panel and diagnostic passes. Native descendants inherit bounded CPU affinity/memory; incomplete budget admissions remain explicit. See [MUMmer4 report](mummer4.md) for the separately authorized comparison, retained adapter failures and licensing/method gates. No route selected.
