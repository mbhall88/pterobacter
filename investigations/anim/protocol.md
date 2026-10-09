# ANIm feasibility protocol — issue #1

Recorded 9 October 2026 before any candidate alignment measurement. This is a bounded development investigation, not production code or biological evaluation. Michael selects the route after review.

Comparator: `reciprocal-nucmer3-mum-1-pyani030-span-v1`, MUMmer 3.23 `nucmer --mum`, `delta-filter -1`. Preserve the comparator unchanged. For each orientation record integer D=sum(reference span+query span), E=sum(delta fifth-header error integer), identity=100(D−2E)/D, and both interval-union coverages. Coverage uses all input bases. Successful empty alignments have null identity and zero coverage; execution or parse failures are errors.

Candidates:

1. Embed the MUMmer 3.23 native stages in one executable, replacing Perl orchestration. Isolate legacy global state and `exit()` in child processes executing the same binary. Do not invoke other executables. Measure native-source rebuild parity against pinned installed tools.
2. Rust minimap2 binding, pinned dependency/lockfile, asm5, asm10 and asm20, base-level CIGAR alignment, secondary alignments disabled. Keep supplementary alignments. Compute E from substitutions plus inserted/deleted bases, D from reference/query spans, and coverage from interval unions. Approximate `dv` and gap-compressed `de` are excluded.
3. Survey other alignment crates for whole-genome anchoring, rearrangement and one-to-one filtering. Stop before building an engine from a pairwise primitive when those capabilities are absent; classify this route as blocked, not equivalent.

## Fixture list fixed before measurements

Deterministic synthetic DNA, seed 20261009, nominal length 120,000 bp:

- identical; reverse complement;
- substitution rates 4%, 4.01%, 4.5%, 5%, 5.5%, 6% (nominal 96/95.99/95.5/95/94.5/94% identity);
- AF boundaries: exact shared prefixes of 79.9%, 80%, 80.1%, with independent residual sequence;
- insertion/deletion tracts; inversion; block permutation;
- duplicated repeat blocks in the reference and query;
- fragmentation into 12 contigs; 2% ambiguous N bases;
- independent random sequences (successful no-alignment);
- invalid alphabet, duplicate identifiers, empty FASTA, missing file;
- deliberate output/engine failure (unwritable or missing output parent).

Also measure one 5,000,000-bp synthetic substitution pair and one full 140-reference synthetic panel of 500,000-bp references, with divergence spanning 0–10%. The panel is a complete measured workload, not an empirical substitute for the real reference panel. Reuse the already exposed P2 real-panel cost as contextual evidence, with its limitations. No new biological reference/query evaluation.

## Equivalence and stopping

Require exact integer D/E, both covered-base counts, whole lengths and status equality in both orientations on every valid fixture; compare exact >96 and >=80 gates separately. Filtered-delta byte parity (excluding filename line) is stronger corroboration for MUMmer. A one-base mismatch fails equivalence. A candidate producing no alignment where the comparator aligns is a discrepancy, not an omitted row.

Report the six numerical differences separately for the nominal 94.5–96% substitution fixtures and the AF boundary fixtures. Nominal mutation rates do not claim measured skani rescue eligibility. Distinguish tested equivalence from proof on arbitrary real genomes.

Stop after this fixed matrix, the genome-sized pair and 140-reference panel; maximum 2 CPU-hours of alignment, 30 minutes wall time per pair, 8 parallel workers, 8 GiB per worker. Stop a candidate on unsafe crashes, inability to build after two focused repairs, or absent whole-genome/filtering machinery; retain evidence and report the blocker. Do not tune presets against results, enlarge fixtures to chase equivalence, run holdout evaluations, select a production route or amend frozen scientific policy.

Record raw alignments, exact argv, exit codes, input/source/binary hashes, time and peak RSS in the research run. Report binary-size increments against an empty release executable; Linux build/runtime, macOS build/runtime honestly including unavailable status; licence source and browser obstacles. The development harness validates FASTA consistently before any backend, separately from native-tool permissiveness. Full-panel timing excludes fixture generation; include setup/parse costs where measured.
