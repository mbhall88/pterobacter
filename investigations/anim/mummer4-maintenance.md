# MUMmer4 maintenance follow-up

Codex GPT-6 (exact backend identifier unavailable), 9 October 2026. Michael
requested further exploration because MUMmer4's integration and maintenance may
outweigh strict upstream compatibility. This extends the [earlier comparison](mummer4.md),
which remains unchanged. No backend or new scientific method is selected.

## Accepted compatibility target

On 9 October 2026, after reviewing the [ambiguity literature and upstream
history](ani-ambiguity-literature.md), Michael instructed us to match MUMmer3
output for now and accept the known long-N risk. Continue with this report's
maintained MUMmer4 candidate, which preserves reference Ns. Retain literal
N-to-N equality, the fifth delta-header error field, the P1 span formula and
inclusive interval-union coverage against all input bases. Do not add a
known-base denominator or substitute similarity errors.

Upstream introduced reference-N randomization in 2020 to prevent very long N
stretches from aligning. Removing it may restore that problem; the existing
fixtures do not establish protection against long N-only anchors. This risk is
accepted for the current compatibility target, rather than a requirement to
change the metric now. If it becomes a practical issue, review seed masking
separately from identity accounting and record any resulting method change.

The existing patch and measurements already implement this target on the
20-pair matrix, so this decision requires no new alignment run. The modified
140-reference panel remains unmeasured. Production backend acceptance,
engineering/platform validation and distribution review remain pending.

**Recommend advancing a small maintained MUMmer4 candidate to review.** Five
changed upstream lines across three files remove reference-N randomization,
honor the delta output stream, and repair the public alignment template. The
modified CLI and library reproduce MUMmer3 on the full existing 20-pair matrix.
The library also supports the tested shared-process concurrency, without stage
symbol manipulation, self-spawn, or output-stream redirection. This evidence
changes the engineering recommendation: strict compatibility does not require
accepting MUMmer3's integration costs on these fixtures.

## Patch and numerical evidence

[Exact edits](mummer4-maintenance.patch) (zero-context diff; use `git apply --unidiff-zero`),
[premeasurement protocol](mummer4-maintenance-protocol.md),
[executor](mummer4_maintenance.py), [shared-process driver](mummer4-maintenance-driver.cc),
and [machine-readable observations](results/mummer4-maintenance.json).

| Comparison | Exact reciprocal pairs | Complete filtered / unfiltered orientations | Gate changes |
|---|---:|---:|---:|
| Modified CLI4 versus retained CLI3 | 20/20 | 40/40; 40/40 | 0 |
| Modified library4 versus modified CLI4 | 20/20 | 40/40; 40/40 | 0 |
| Modified CLI4 versus retained upstream CLI4 | 19/20 | 39/40; 39/40 | 0 |

Equality covers status, D/E, covered bases and whole-input lengths in both
orientations, plus complete context/header/offset blocks. Four invalid inputs
are rejected per route; no-alignment retains null identity and zero coverage.
Library measurements use actual filtered files, resolving the earlier primary
adapter-validation gap. The 140-reference panel was **not** repeated with this
modified build; previous upstream4 panel observations remain historical controls.

The original ambiguity fixture now yields D240000/E2400, identity98%, AF100/100
in both directions, matching MUMmer3. Removing reference substitution accounts
for the deliberate difference from upstream4's E1789/98.50916667% direction.
This is a changed upstream method, not an upstream4 equivalence claim. These
fixtures do not establish equivalence for arbitrary ambiguity tracts or tied inputs.

## N–N identity remains a scientific choice for both versions

[Upstream issue51's last reply](https://github.com/mummer4/mummer/issues/51#issuecomment-619080793)
distinguishes exact-character identity from positive-score similarity. A new
diagnostic compares the original ambiguity query against itself: the same
120-kb bytes, with 2,400Ns, no generated or biological sequence additions.
This is the stated reason for one new reciprocal CLI3 comparator execution.

| Route | Identity errors / identity | Similarity errors / positive-score percentage |
|---|---:|---:|
| MUMmer3 | 0 / 100% | 2400 / 98% |
| Upstream MUMmer4 | 2400 / 98% | 2400 / 98% |
| Modified MUMmer4, reference Ns preserved | 0 / 100% | 2400 / 98% |

Both directions agree in this self-comparison. Preserving Ns reproduces
MUMmer3's exact-character treatment; it does not make unknown bases positive
biological evidence. Using similarity errors instead would alter the frozen
column-five method. This diagnostic originally left that choice open; Michael's
subsequent decision above retains MUMmer3 identity and unchanged
coverage/denominators. The earlier preference for MUMmer3 should therefore be read
as a compatibility preference, not a claim that it solves all ambiguity semantics.

## Shared-process concurrency

One process invokes separate aligner objects on fragmented, repeat_reference,
ambiguous and genome_5mb; both orientations, two repetitions, one native query
thread per call. The identical 16-call workload runs once with one worker and
once with eight. **All32 calls match the modified-library matrix**, including
complete unfiltered and filtered blocks. No child aligner executables are used.

| Workers in one process | Batch wall / CPU seconds | GNU peak RSS |
|---|---:|---:|
| 1 | 23.95 / 23.73 | 81.97MiB |
| 8 | 6.65 / 24.48 | 329.93MiB |

This single within-run measurement demonstrates useful concurrent execution for
the fixed workload. It is not a panel/laptop estimate, a statistical performance
ranking, a race-detector result, or proof of arbitrary-input reentrancy. Four
long genome calls dominate this workload; it does not measure eightfold scaling.
Historical pair timings used other builds/harnesses and are not a controlled
comparison to this PIC build.

Reference parsing no longer requires the RNG mutex, and output is object-local.
The existing filter still uses libc rand for tied LIS choices. Our bridge
serializes/reseeds the complete filter section. Other host code using rand/srand
does not honor that mutex; isolate filter RNG before production integration.
Concurrency is therefore improved and demonstrated on this subset, with a
remaining shared-state boundary.

## Maintenance and next review

The three upstream fixes and smaller bridge are a more attractive maintenance
starting point than five embedded legacy stages and GNU symbol handling. Carry
the patch against the pinned source, record a distinct build/profile identity,
and verify it on upgrades. No maintainer contact or permanent fork was created.

Before accepting a production route:

- Use the agreed MUMmer3 output target and keep the P1 comparator and historical
  results. Bind a reviewed candidate build/profile; parity on other IUPAC symbols
  and arbitrary masked tracts remains unproven. The known long-N risk is accepted
  for now, with future fixes subject to explicit compatibility review.
- Isolate filter RNG and audit native fatal errors/worker exceptions, cancellation
  and resource bounds. The outer catch does not contain upstream exit calls or
  exceptions escaping native worker threads.
- Replay the modified full panel and difficult/tied/ambiguity cases; test actual
  macOS builds/runtime. Linux/GCC PIC compilation is demonstrated here, macOS
  and WASM remain unverified.
- Resolve the previously identified Artistic2.0/GPL-noticed component distribution
  questions. This patch does not remove those components or clear licensing.
- Map any accepted method to P1/P4a-r3 and #8/#25/#30 before production/evaluation.
  #1 remains open; no threshold, label, frozen contract, merge or release changes.

## Provenance and limits

Fresh private research run:
`Enterobacter/results/tool-planning/2026-10-09/terobacter-issue1-mummer4-maintenance-codex/`.
Pin remains v4.0.1/eb734606f2d516f42a0e0dce7a116bfb88ec1ebf. Native build is
GCC C++17/O3/NDEBUG/PIC, static library, OpenMP disabled. Only CLI/filter targets
are built; configure warns that `--without-swig` is unrecognized. No SWIG target
is built. The existing Rust probe linker still selects non-PIE; the C++ concurrency
driver links successfully using the PIC objects without that workaround.

The first Rust build omitted its shared FASTA-validator snapshot and failed
before alignment. One focused packaging repair copied the unchanged validator;
failed logs and repair hashes are retained. No numerical upstream repair was
added after seeing results. The final executor adds explicit matrix/invalid-input
stop checks; those same conditions were independently checked on this run.

The ledger descends from a hash-bound read-only copy of the prior ledger:759.29s
carried forward, **110.33s new conservative charge / 77.40s measured CPU**, total
869.63s of the original7200s. No outstanding reservations or cleanup breaches.
The diagnostic batches share a90s deadline with widths1/8 and6GiB total memory
per process tree. Original ledger/results remain unchanged; do not fork a second
active ledger descendant. Twenty existing Python tests pass; new Python compiles
and the C++ driver passes Wall/Wextra/Werror syntax checks. Both earlier artifact
manifests verify unchanged:15,734 original artifacts and7,608 upstream4 artifacts.
