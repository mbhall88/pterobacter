# MLST feasibility protocol — issue #2

Recorded 9 October 2026 before any candidate or comparator measurement. This is a bounded development investigation, not production code or biological evaluation. Michael selects the route after review. Covers gate E-MLST and the evidence side of AC08.

## Comparator

`mlst 2.32.2` + `blastn 2.16.0+` + `any2fasta`, from the pinned `mlst` conda environment, run on the historical scheme package (PubMLST *Enterobacter* scheme, submissions through 31 December 2024; retrieved 2026; not a 2024 byte snapshot). Fixed invocation: `--scheme ecloacae --datadir <package>/pubmlst --blastdb <index built from the package> --minid 95 --mincov 50 --threads 1 --full --json --debug`. The comparator is preserved unchanged and is not an acceptable runtime dependency.

Rules read from the `mlst` 2.32.2 source (before measurement) that a candidate must reproduce or explicitly diverge from:

1. `blastn`: `-ungapped -dust no -word_size 32 -perc_identity 95 -evalue 1E-20 -max_target_seqs 100000`, megablast scoring, assembly contigs as query, alleles as database. So hits are **ungapped HSPs**, a gapped alignment becomes two or more partial HSPs, and short HSPs fail the e-value filter. N and other non-ACGT bases are mismatches.
2. A hit is discarded unless `nident / allele_length >= mincov/100` (identity-weighted coverage, not alignment length).
3. Exact: HSP length = allele length = nident. Each further exact hit at the same locus appends `,<id>` (a second copy of one allele gives `n,n`; two alleles give `a,b`); an exact hit replaces an earlier `~`/`?` token; later hits never replace an exact token.
4. Inexact full-length ungapped HSP gives `~<id>`; partial gives `<id>?`. The first such hit in BLAST output order wins (`||=`), not the best allele.
5. A missing locus is `-`. ST is looked up only from the exact signature (`-` otherwise).
6. Score = `int((7 - 0.3 n(~) - 0.5 n(?) - 1.0 n(-)) * 90 / 7) + (10 if ST found)`. STATUS: `PERFECT` (ST found), `NONE` (score <= 0), `NOVEL` (no non-numeric token), `MIXED` (any `,`), `MISSING` (any `-`), `BAD` (score < 70), else `OK`.
7. Importer-level states and the profile-compatible candidate ST follow the existing Enterobacter importer: ambiguous (any `,`), incomplete (any `-`), inexact (any `~`/`?`), exact-profile match or unmatched profile; a candidate ST needs each repeated exact allele to be a confirmed copy at distinct locations. Candidates must therefore emit exact-hit locations.

## Candidates

1. **minimap2** through the Rust binding crate, pinned dependency and lockfile. Contigs indexed, alleles as queries, base-level CIGAR, secondary alignments retained. Presets `asm5`, `asm10`, `asm20` only, all with the same fixed secondary-alignment settings; no other tuning. Alignments are converted to ungapped HSPs (split at indels, trim terminal mismatches) and fed to the shared comparator-rule layer.
2. **Exact-allele hash plus alignment crate.** Exact two-strand allele lookup (Aho–Corasick), fixed-k seeding of inexact and partial candidates, then a pairwise aligner on seeded windows for the best 32 alleles per window (rank by shared seeds, ties by allele id). Primary aligner `bio` (rust-bio) pairwise; second `block-aligner` if it builds within the repair limit. Same HSP conversion and rule layer. An exact-only matcher is reported separately and is never presented as parity.
3. **Survey** of other crates (licence, maintenance, WebAssembly, whether it provides seeding/alignment/filtering). A crate that only supplies a pairwise primitive is classified as such; measure it only if it plausibly changes the recommendation.

The shared rule layer (items 2–7 above) is common to all candidates so that the engines are compared on hits, not on re-implemented scoring. Hit-level agreement with the comparator's `--debug` rows is a stronger, separate corroboration.

## Fixture list fixed before measurements

Deterministic synthetic assemblies, seed 20261009: random background (GC about 55%) with allele sequences from the package embedded. Development fixtures only; no genome, read set or real isolate is used. All fixtures are rebuilt from the package by script and are not committed.

- exact: ST1, ST2480, ST3295 and one mid-table ST; all seven alleles forward; all reverse-complemented; strands mixed;
- exact seven alleles that form no recorded profile (unmatched);
- missing locus (one, three, all loci absent);
- inexact full length: one substitution in the middle; one at 1, 3 and 10 bases from an allele end; 4.9% and 5.1% substitutions (identity 95.1/94.9); five loci at once;
- tied nearest alleles: a variant of two alleles that differ by one base, to test which allele id is displayed;
- partial: allele truncated to 70%, 51%, 49% and 30% of its length, and a 40-base fragment (e-value floor);
- contig edges: allele ending exactly at contig end (exact); allele with 30% missing past the contig end; allele split across two contigs; allele in a one-contig-per-allele assembly;
- indels: 1-base insertion and 1-base deletion near the middle, 3-base insertion, 1-base indel at 10 bases from an end (ungapped split rule);
- duplicates: the same allele twice; two different exact alleles at one locus; exact plus inexact at one locus in both orders; one allele present in a diverged (90%) second copy;
- ambiguous bases: one N inside an allele; 2% N; N at an allele end; IUPAC code (R/Y); lowercase/soft-masked allele;
- 12-contig fragmentation of the exact ST1 assembly;
- no alleles at all (random contigs): successful `NONE`;
- invalid inputs: empty FASTA, missing file, non-nucleotide sequence, duplicate contig identifiers, unwritable output parent. The development harness validates FASTA consistently before any backend, separately from the comparator's permissiveness.

Performance workloads: one 5,000,000-bp exact-ST assembly in one contig and in 200 contigs; a panel of 20 such assemblies with substitutions in a subset of alleles. Reported: wall time, CPU time, peak RSS, and added release-binary size against an empty release executable. Real-reference-panel cost from earlier exposed work may be quoted as context only.

## Equivalence and stopping

Parity per fixture requires exact equality of: every locus token, ST, STATUS, SCORE, scheme, importer state, candidate ST and the set of exact-hit locations, for the comparator and a candidate. Report hit-level agreement (locus, allele id, contig, start, end, strand, nident, alen) separately, and report discrepancies per fixture rather than as a pooled rate. A candidate that reports no hit where the comparator has one is a discrepancy, not an omitted row.

Stop after this fixed matrix and the performance workloads. Limits: 1 CPU-hour of alignment in total, 10 minutes wall time per assembly, 4 parallel workers, 2 GiB per worker (this host has about 3 GiB). Stop a candidate on unsafe crashes, inability to build after two focused repairs, or absent whole-sequence seeding/filtering capability; retain evidence and report the blocker. Do not retune presets or seed settings against results, enlarge fixtures to chase parity, add real genomes, run holdout evaluations, select a production route or amend frozen scientific policy.

## Recorded for every run

Exact argv, exit codes, input/source/binary hashes, time and peak RSS; hit tables from the comparator and each candidate. Build and runtime on Linux; macOS stated honestly as unavailable if untested; licences of each dependency and of the scheme data (PubMLST terms, section 3, records submitted through 31 December 2024 with scientific citation); browser-portability obstacles.

## Addendum A (9 October 2026, after comparator-only runs, before any candidate was run on the full matrix)

The fixed fixtures were run through the comparator alone and its `--debug` hit rows read. This shows three rules the source reading above missed, so the shared rule layer implements them and candidates are not penalised for not guessing them. None changes the fixture list, stop rule or presets.

1. **Report order.** Hits come out per contig in input order, then per subject by best HSP score descending, ties by database order **descending** (the highest allele id first among equal scores). `||=` therefore shows the highest-id allele among the best-scoring ones. Rule layer sorts HSPs the same way; the 32-allele selection in route 2 ranks by shared seeds then allele id descending (amends "ties by allele id").
2. **HSP trimming.** Ungapped extension scores +1/−2 and keeps the first maximum: a substitution followed by two or fewer matches is trimmed off, so a base change 1–3 bases from an allele end is a partial hit (`n?`), not `~n`. X-drop was first taken as 10 (blastn's 20 bits at lambda 1.28); see addendum B for the corrected value.
3. **Seed.** An HSP needs an exact 32-base run; `-perc_identity 95` applies to the HSP, not the allele. The e-value filter is inert here: the shortest allele is 249 bases, so a passing hit has at least 125 identical bases, far above the score needed for 1E-20.

**Route D added.** Because the comparator never computes a gapped alignment, route 2's alignment crate and route 1's gapped alignments only nominate diagonals; the ungapped finishing is the same shared code for every route. Route D is the same finisher driven by exact 32-mer seeding of the allele set, with no alignment library. It is measured on the identical fixtures and stop rule as a third route found necessary by the comparator analysis, not a replacement for routes 1–3.

**Disclosure.** During development of the shared rule layer, route D was run on eight fixtures (`exact_st1`, `inexact_mid`, `partial_51`, `indel_ins1`, `tie_two_alleles`, `dup_two_alleles`, `inexact_end3`, `amb_N1`) and matched. The rule layer was written from the comparator source and the observations above, not adjusted after a mismatch. Matches on this fixed matrix show tested equivalence, not proof for arbitrary assemblies.

Parity for the importer-level state, candidate ST and exact-hit locations is derived by the harness from allele tokens and exact-hit locations identically for the comparator and every candidate. Host limits: 2 GiB per worker; the host has about 3 GiB.

## Addendum B (9 October 2026, still before the minimap2 and exact+alignment matrices)

Hit-level comparison of the shared finisher with the comparator exposed one wrong derivation in addendum A: the raw X-drop. My first value (10, from blastn's documented 20 bits at lambda 1.28) truncated an HSP the comparator extended (rplB 378 against ST1 rplB, 598/607 identical); a larger value (20) extended one the comparator stopped (fusA 354, 550/552). Two natural probes bracket the raw value to 12–14. Running all 52 fixtures, X-drop 12, 13 and 14 each give zero differences against all 100,002 comparator HSPs that pass mincov (11 and 15 each break one HSP per fixture). The midpoint, 13, is used. This corrects a comparator-model constant shared by every route; it is not a candidate preset and was not adjusted for any route's parity. Token-level parity of the fixed matrix was already 52/52 at 10, so the correction changes hit-level, not call-level, agreement.

## Addendum C (9 October 2026, recorded while summarising; the fixed matrix had been run)

- **Correction.** Protocol and addendum A say the host has about 3 GiB. That was a misread: `free` reports roughly 3 TB on this shared node. The limits (4 workers, 2 GiB per worker, 1 CPU-hour, 10 minutes per assembly) stand as written; no run exceeded 170 MiB or 2 seconds, and total CPU time was about 0.1 CPU-hours.
- **Order rule check.** The addendum A report-order rule holds for every adjacent pair of hits for the same locus in all 52 fixtures (0 violations in 104,913 pairs). Order between different loci at equal score is not reproduced, and does not matter because tokens are decided per locus.
- **minimap2 binding crash.** Route 1 as pre-registered (in-memory contig index) segfaulted (exit 139) on every multi-contig assembly, 8 of 52 fixtures, at every preset. The stop rule says to stop a candidate on an unsafe crash; the in-memory result is therefore reported as route 1's measured outcome. A file-indexed variant (contigs written to a temporary FASTA, indexed by minimap2's own reader) is reported as a clearly labelled supplementary deviation, because it is a different code path and the only way to learn whether the engine itself is usable. The pure-Rust translation `minimap2-pure-rs` (route 1b) was added after seeing the crash and measured with identical presets and settings.
- **block-aligner.** Offers global and X-drop alignment but no local mode, so it is used as an anchored X-drop extension from one seed per allele in each direction. Built first time with one repair, to my own adapter (wrong anchor offset), not to the build.
- **Workloads.** The performance workloads are the 5 Mbp single-contig and 200-contig assemblies and a 20-assembly synthetic panel; there is no 140-reference panel for MLST.

## Addendum D (9 October 2026, after coordinator review of the first report)

Four changes, none altering the fixture list, presets or any reported measurement:

- **Stop controls.** The harness now enforces the stop rules above (wall limit, memory limit, worker cap, CPU budget, stop on first crash, retained failure records), tested with synthetic subprocesses. The earlier saved runs predate these controls; they were not repeated.
- **Retained diagnostics.** `harness/xdrop_sweep.py` and `harness/order_audit.py` re-derive the X-drop bracket and the report-order check from saved comparator output (route D rerun at ten X-drop values, about 1.7 CPU-minutes). The retained order audit counts 102,827 pairs (not 104,913): it groups by contig and locus and leaves out the invalid duplicate-contig-id input. The probe gained a diagnostic `--xdrop` option; the default and every reported result still use 13.
- **Importer check.** `harness/importer_diagnostic.py` passes saved comparator and route D outputs through the real, unchanged Enterobacter importer, with reconstructed (not historical) receipts and the comparator's prepared command for route D. It does not give route D an execution receipt.
- **Size baseline.** The "core" binary is the route-D-capable baseline, not an engine-free build; the column is renamed.
