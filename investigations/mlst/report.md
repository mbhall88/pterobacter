# MLST feasibility report — issue #2

Written 9 October 2026 against the [protocol](protocol.md) (addenda A–C record what changed after the first comparator runs). Development and synthetic fixtures only; no production code, no real genome, no route selected. Michael chooses. Covers gate E-MLST and the evidence side of AC08.

## Result

**A self-contained historical-MLST matcher is feasible, and the simplest route reproduced the comparator best.** The comparator (`mlst` 2.32.2 + BLAST+ 2.16.0) never computes a gapped alignment: it runs `blastn -ungapped -word_size 32` and applies its own rules to the ungapped HSPs. Reproducing it therefore needs exact 32-mer seeding, BLAST's ungapped X-drop extension and the `mlst` rules. It does not need minimap2 or a pairwise-alignment crate.

| Route | Fixture call parity (52 valid) | HSP-level agreement | Notes |
|---|---|---|---|
| **D** exact 32-mer seeds + ungapped extension (pure Rust, no alignment crate) | **52/52** | **all 100,002 comparator HSPs, 0 extra** | added after the comparator analysis (addendum A) |
| 2 exact hash + seeds + `bio` local alignment | 52/52 | exact hits only by design | alignment only nominates diagonals |
| 2 exact hash + seeds + `block-aligner` X-drop | 52/52 | exact hits only by design | one repair, to my own adapter |
| 1 minimap2 C binding, in-memory index (as pre-registered) | 44/52 at asm20 (43 at asm10, 42 at asm5) | — | **SIGSEGV on all 8 multi-contig fixtures** at every preset |
| 1\* minimap2 C binding, file index (supplementary deviation) | asm20 52/52, asm10 51/52, asm5 50/52 | 148 / 1,311 / 21,315 comparator HSPs missing | |
| 1b `minimap2-pure-rs` (pure Rust, added after the crash) | asm20 52/52, asm10 51/52, asm5 50/52 | identical to 1\* | LLM-assisted translation, self-described experimental |
| exact-only (no inexact handling) | 25/52 | — | **not parity**; every `~`, `?`, indel, edge and N fixture fails |

"Parity" means identical allele tokens, ST, STATUS, SCORE, importer state, profile-compatible candidate ST and exact-hit locations (contig, start, end, strand). Per-fixture results: [`results/parity-per-fixture.tsv`](results/parity-per-fixture.tsv). This is tested equivalence on 52 designed fixtures, not proof for arbitrary assemblies.

## Recommendation (for Michael to decide)

Build route D as the in-binary matcher, as a **new, named engine** (ungapped seed-and-extend reproducing blastn HSP semantics), not as "mlst 2.32.2". The frozen design says successful fixtures alone do not allow relabelling a new engine as the old one, so it needs the versioned amendment and a mapped effect on the executable/profile pins before #25 (spec, "Frozen design SHA256 ... P4a-r3"). Keep `mlst` + BLAST+ as the comparator and as the reference for #25 RG-4 on real assemblies.

Why not the others:

- **minimap2 (C binding)**: no fidelity gain, a crash path in the binding's in-memory multi-record index (probable cause in [Findings](#findings)), a dynamic `libz` dependency, and lower sensitivity at asm5/asm10 where the 5% spread-substitution fixtures have no seeds. asm20 matches the fixtures but missed 148 comparator HSPs, which could matter when the first inexact hit decides a token.
- **`minimap2-pure-rs`**: same results as the C library; removes the C and zlib issues and is WebAssembly-shaped, but costs +530 kB, needs a file path to build its index, and says of itself that it is an experimental translation to be used with vigilance.
- **Exact hash + alignment crate**: matched, but the aligner contributes nothing the comparator uses; the parity comes from the ungapped finisher. It adds a dependency and, in the exact-hash route, a second code path for the same decision.
- **Exact-only**: must not be presented as parity (27 of 52 fixtures differ).

Costs of D to accept: it reimplements behaviour of BLAST and `mlst` that was read from `mlst`'s GPL-2 source and observed from its output, so a clean-room/licence view is worth taking (no code was copied); and its constants (X-drop 13, order rule) were fitted to the comparator on these same fixtures, so they are in-sample.

## What the comparator does, and what is reproduced

Rules from the source (protocol items 1–7) plus three found by running it (addenda A, B):

| Rule | Reproduced |
|---|---|
| `blastn -ungapped -dust no -word_size 32 -perc_identity 95 -evalue 1E-20 -max_target_seqs 100000`, megablast +1/−2 | yes: 32-base exact seed, +1/−2, X-drop **13** (bracket 12–14, see below), identity filter per HSP |
| hit kept if `nident / allele length ≥ 0.5` | yes |
| exact: full length, all identical; further exact hits append `,id`; an exact replaces an earlier `~`/`?` | yes |
| `~id` full-length ungapped with mismatches; `id?` shorter; first in BLAST order wins | yes |
| BLAST order: contig, then subject best score, ties **highest allele id first** | yes; 0 violations in 104,913 within-locus adjacent pairs; order across loci not reproduced and irrelevant |
| a substitution followed by ≤2 matches is trimmed off (so a base change 1–3 from an allele end is `n?`) | yes |
| e-value floor | modelled, inert: shortest allele is 249 bases so a kept hit has ≥125 identical bases |
| score `int((7 − .3 a − .5 b − c)·90/7) + 10 if ST`, STATUS table | yes; unit-tested incl. BAD (BAD is not reached by any fixture) |
| N and IUPAC codes are mismatches; lower case is upper-cased | yes (`amb_*`, `lowercase`) |
| duplicate tolerant input: duplicate contig ids and protein FASTA are accepted by `mlst`; empty, headerless and missing files exit 1 | **diverged on purpose**: the harness rejects all five (exit 2) before any backend, as the protocol specifies |

X-drop: the documented 20 bits did not convert to a value that reproduced the comparator (10 truncated one HSP, 20 extended another). Two natural probes bracket the raw value to 12–14; 12, 13 and 14 each give zero differences over all 100,002 HSPs, 11 and 15 each break one HSP per fixture. I used 13. This is a comparator-model constant shared by every route, not a candidate preset.

Not modelled, because no fixture exercised it: BLAST's removal of HSPs with common endpoints, query-splitting for very large queries, soft-masking of subjects. Real assemblies with diverged paralogs are the likely place to find a difference.

## Fixture coverage

52 valid fixtures plus 5 invalid inputs ([protocol](protocol.md#fixture-list-fixed-before-measurements)). Comparator STATUS across the valid fixtures: 27 OK, 14 PERFECT, 6 MISSING, 3 MIXED, 1 NOVEL, 1 NONE. Covered: exact and reverse-complement, mixed strands, four STs, unmatched exact profile, missing loci, inexact at 1/3/10 bases from each end, 95.1%/94.9% identity, ties between near-identical alleles, partial 70/51/49/30%, 40- and 60-base fragments, alleles at and across contig ends and split over two contigs, one-contig-per-allele, 12-contig fragmentation, indels, duplicates (same allele twice and on both strands, two alleles, exact before/after inexact, diverged 90%/96% copies), N at three positions, IUPAC R, lower case, random assembly. Not covered: STATUS `BAD`, real draft assemblies, paralogs, very fragmented assemblies, scheme loci beyond *Enterobacter*.

## Cost

Linux x86-64 (AMD EPYC, 8 cores), one thread, process wall time including parsing and index building, fixture generation excluded. Total alignment CPU across all measurements was about 7 CPU-minutes, so no stop limit came near.

| Route | 5 Mbp, 1 contig: wall / peak RSS | 5 Mbp, 200 contigs: wall | 20-assembly panel: total wall | binary +vs empty / vs core |
|---|---|---|---|---|
| comparator (`mlst` + Perl + BLAST+ + any2fasta) | 1.08 s / 162 MiB | 1.07 s | 24.5 s | — |
| D ungapped | 0.38 s / 107 MiB | 0.29 s | 7.4 s | +164 kB / 0 |
| exact + `bio` | 0.42 s / 12 MiB | 0.87 s | 13.5 s | +236 kB / +72 kB |
| exact + `block-aligner` (AVX2) | 0.43 s / 12 MiB | 0.69 s | 11.2 s | +244 kB / +80 kB |
| minimap2 C, asm20 (file index) | 0.62 s / 80 MiB | 0.68 s | 14.1 s | +343 kB / +178 kB |
| `minimap2-pure-rs`, asm20 | 0.94 s / 64 MiB | 0.57 s | 11.0 s | +695 kB / +531 kB |

All routes except exact-only gave the comparator's call on all 22 performance assemblies (exact-only fails the 200-contig one, where cuts split alleles). Runtime is not a reason to avoid BLAST; the reason is the external executables. "Core" is the probe with JSON output and no engine; the empty-executable baseline is 302 kB. The exact routes build a k-mer seed index only when some locus has no exact allele, which is why they use 12 MiB on an all-exact assembly. [`results/performance.tsv`](results/performance.tsv), [`results/binary-sizes.tsv`](results/binary-sizes.tsv).

## Licences and data provenance

- **Software.** All crates in every variant are permissive (MIT, Apache-2.0, Zlib, Unlicense or dual combinations; list in [`results/dependency-licences.tsv`](results/dependency-licences.tsv)); the vendored minimap2 C source is MIT. `mlst` 2.32.2 is GPL-2.0, `any2fasta` 0.8.1 GPL-3.0 and BLAST+ 2.16.0 NCBI public domain (conda package metadata); the first two are used only as comparators, never linked or shipped. The GPL question above applies to rules read from source, not to code.
- **Scheme data.** PubMLST *Enterobacter* scheme, package `enterobacter-pubmlst-pre2025-retrieved-20261006`: 4,751 alleles, 3,291 profiles, retrieved 2026 from the anonymous export, which the server limits to submissions through 31 December 2024. The current PubMLST terms (section 3, updated 13 May 2026) permit use and redistribution of records submitted through that date with scientific citation and acknowledgement. It is a 2026 retrieval, not a 2024 byte snapshot; three profiles and one allele present in a 2022 archive are no longer exported, and completeness is not claimed. This comes from the earlier packaging task's findings and was not re-fetched here. File hashes are in the package manifest (e.g. `dnaA.tfa` sha256 `cb78a1c2…2b2a`). Output must carry the cutoff limitation text defined by the existing importer; the engines do not touch it. The package is not committed here.

## Portability

- **Linux.** Built and ran all variants with Rust 1.95.
- **macOS.** Not run. `cargo check` for `aarch64-apple-darwin` and `x86_64-apple-darwin` passed for the core, `bio` and (x86-64) `block-aligner` variants. `block-aligner` needs a per-architecture SIMD feature (AVX2 here; aarch64 needs `simd_neon`), so its aarch64 check failed as configured. The minimap2 C build could not be checked: there is no macOS C toolchain on this host, so that result is "unavailable", not "fails".
- **Browser.** The `wasm32` target is not installed here, so none was built. Route D, `bio` and `block-aligner` (`simd_wasm`) are pure Rust with no filesystem need and look WebAssembly-compatible. Obstacles: the allele seed index costs about 100 MiB at start (it could be precomputed into the package); the minimap2 C binding needs a wasm C toolchain plus zlib; both minimap2 variants as used here need a temporary file to index the contigs; `mlst`/BLAST are not an option.
- **Self-containment.** The minimap2 variant links `libz.so` dynamically; a self-contained build would need static zlib. Routes D and 2 have no native dependencies.

## Findings

- The minimap2 binding's `with_seqs_and_ids` passes a `Vec<CString>` pointer where a `*const *const c_char` array is expected (its own doc comment says it works for one sequence but not several), which fits a crash on every assembly with more than one contig. I did not confirm the cause under a debugger. Indexing from a file avoids it.
- At asm5/asm10 minimap2 misses the two spread-substitution fixtures (one base change about every 20 bases, no 19-mer seeds); the comparator needs a 32-mer seed, so its own answer there rests on a different allele (`~198`) or a partial hit (`1?`). This is the comparator's sensitivity floor, and D shares it by construction.
- The allele id shown for `~` and `?` is not a true best allele. It is the highest id among equal-scoring alleles, so `733?` or `772?` appear for a truncated allele 1 whose prefix is shared by many alleles. Any route must score the whole allele set to reproduce it; "top 32 by shared seeds, ties by id descending" did so on these fixtures.
- `mlst` accepts duplicate contig identifiers and protein FASTA silently (exit 0, STATUS `MIXED`/`NONE`); the harness rejects them.

## Limits

- Fixtures are synthetic and mine; the rule constants were fitted on them (addenda A, B). Equivalence is tested, not proved.
- One scheme (*Enterobacter*), one locus family, 249–648-base loci.
- Candidate engines were measured on one run each; times are indicative.
- Route D was run on eight fixtures during development before the full matrix (disclosed in addendum A).
- Deviations from the protocol: minimap2 file-index variant and `minimap2-pure-rs` added after the crash; route D added after comparator analysis; X-drop corrected; host-memory misstatement corrected (addendum C).

## Decisions for Michael

1. Route: D, or another? If D, who owns the amendment that names it as a new engine?
2. Whether to take a licence view on rules derived from GPL-2 `mlst` source.
3. Whether the scheme package ships inside the binary (about 2.4 MB of FASTA and profiles) or is fetched, given the PubMLST terms above.
4. macOS and wasm checks need a host this run did not have.

## Reproduce

```bash
export MLST_ENV=<prefix of the env holding mlst 2.32.2 and BLAST+ 2.16.0>
python3 harness/make_fixtures.py <scheme-dir>/ecloacae FIXTURES       # seed 20261009
python3 harness/run_comparator.py <package-dir> FIXTURES RUNS/comparator
harness/build_variants.sh --check-macos
python3 harness/run_candidate.py probe/target/core/release/mlst-probe ungapped <scheme-dir>/ecloacae FIXTURES RUNS/cand/ungapped
python3 harness/compare.py <scheme-dir>/ecloacae RUNS/comparator RUNS/cand/ungapped RUNS/cmp_ungapped
python3 harness/run_perf.py <package-dir> FIXTURES RUNS/comparator bins.json RUNS/perf.json
python3 harness/summarise.py RUNS results && python3 harness/sizes_licences.py
cd probe && cargo test --release                                       # 16 unit + 5 behavioural tests
```

`<package-dir>` contains `pubmlst/ecloacae/{dnaA..rpoB}.tfa` and `ecloacae.txt`; the comparator reads it through `--datadir`. Raw alignments, exact argv, exit codes, hashes, time and peak RSS for every run are kept in the research run, not in this repository.
