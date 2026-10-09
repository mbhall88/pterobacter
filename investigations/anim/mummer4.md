# MUMmer4 continuation: comparison for backend review

Codex, GPT-6 (exact backend identifier unavailable), 9 October 2026. This separately authorized extension follows [Astra's full continuation brief](https://github.com/mbhall88/terobacter/issues/1#issuecomment-6073763436) and TERO-SPEC-r1.3. It resumes PR18 and preserves the original protocol, inputs, measurements and minimap2 evidence. **No backend is selected.**

MUMmer4 offers a workable, smaller library bridge and faster execution on the measured 5-Mb pair. It does **not** reproduce the frozen MUMmer3 method on every fixture: v4.0.1 replaces reference `N` bases with pseudorandom bases. It also retains global RNG and output-stream behavior in the complete pipeline, and component licensing needs resolution before distribution. Recommend retaining embedded MUMmer3 as the compatibility route pending Michael's decision; consider MUMmer4 only with a reviewed new method profile and distribution plan.

## Numerical comparison

| Route | Exact matrix pairs vs MUMmer3 | Exact panel pairs vs MUMmer3 | Matrix ANI/AF gate changes |
|---|---:|---:|---:|
| Embedded MUMmer3, retained | 20/20 | 140/140 | 0 |
| MUMmer4 v4.0.1 CLI | 19/20 | 140/140 | 0 |
| Embedded MUMmer4, retained stdout reparse | 19/20 | 140/140 | 0 |
| Minimap2 asm5, retained | 8/20 | 16/140 | 5 |
| Minimap2 asm10, retained | 8/20 | 23/140 | 4 |
| Minimap2 asm20, retained | 8/20 | 27/140 | 4 |

The library matches CLI4 on **20/20 matrix and 140/140 panel pairs**, including exact D/E, both covered-base counts, lengths and statuses in both biological orientations, and complete filtered blocks. Original minimap2/comparator alignments were not rerun. [Machine-readable observations](results/mummer4-comparison.json) include directional integers, continuous differences, separate nominal rescue-band/AF-boundary rows and exact gates. Nominal synthetic divergence does not establish skani rescue eligibility.

The ambiguity fixture is the only numerical difference. In QR both methods give D240000/E2400, identity98%, AF100/100. In RQ MUMmer3 gives E2400 while CLI4/library4 give E1789, identity98.50916667%, AF100/100. The pinned `src/umd/nucmer.cc` calls `replace_n_random_letter` during reference parsing. Replay of libc's seed1 sequence shows 611 of2400 replacements match the opposing base, explaining E1789 exactly. This is native-reference dependent and is not an output-order effect; no flags or sequence bytes were tuned for parity. A caller sharing libc RNG state could obtain another result.

All filtered/unfiltered complete blocks match MUMmer3 except this error-count header. Structural comparison sorts **whole context/header/offset blocks**, preserves offset order and multiplicity, and retains raw hashes. The fixed 1/2/4-thread checks preserve complete blocks and filter selections on fragmented/repeated/genome-sized fixtures, two reciprocal invocations per setting. All36 thread-check pairs match primary CLI4. Six directions with two independent library objects each also match; construction and filtering serialize their global-state sections. These fixtures do not prove arbitrary tied-input, thread, compiler or libc determinism.

All four invalid cases are rejected: shared guard before CLI, Rust guard before embedding. Successful no-alignment yields null identity/zero AF. Native output failure returns nonzero; empty-PATH aligned/no-alignment checks pass. Missing/failed measurements remain distinct from successful negatives.

## Adapter failures and corrected execution

The first library pass executed the actual aligner and exact one-to-one filter successfully, but all320 primary orientation records initially had parser errors: `DeltaGraph::outputDelta(ostream&)` ignores its argument and writes global `cout`. Empty expected filter files and complete filtered stdout are retained. A **separate analysis pass** parses that stdout; the original errors and bytes are unchanged. No primary alignment was repeated to fix the parser's input path.

The final bridge redirects/restores `cout` under the same mutex used for `srand(1)` and filtering. Its repaired file adapter is exercised on the remaining fixed thread subset, independent-object and runtime checks. Earlier thread1/2 results also use the separately retained stdout reparse. Full primary matrix/panel execution with the final file adapter was not repeated. This limitation is explicit: recovered native numerical parity is established, but a production adapter still needs fresh end-to-end replay.

The first build failed because public `align_file` cannot instantiate its trampoline with GCC11. The first focused repair uses public `thread_align_file`, also used by CLI4. The second repairs non-PIC static-object linking by making the Linux development executable non-PIE. Both failed builds are retained. The later stream adapter repair changes no upstream numerical source. Production should rebuild PIC objects and contain native fatal errors; this probe is not R3 implementation.

## Engineering costs

Single cluster measurements, Linuxx86_64, Slurm8CPUs/64000MiB. The new harness uses explicit CPU affinity and10ms process-tree polling; historical measurements used the earlier harness. Costs are descriptive and are not an uncontended comparative benchmark or laptop prediction.

| Route | 5-Mb reciprocal wall/CPU seconds | Max GNU RSS MiB | 140-reference wall/CPU seconds |
|---|---:|---:|---:|
| MUMmer3 CLI, retained | 15.14/14.77 | 158.06 | 36.54/211.68 |
| Embedded MUMmer3, retained | 13.46/13.30 | 83.04 | 31.14/178.13 |
| MUMmer4 CLI | 7.22/7.12 | 84.93 | 35.15/162.76 |
| Embedded MUMmer4, initial stream adapter | 7.39/7.30 | 83.13 | 34.61/159.18 |
| Minimap2 asm5/10/20, retained | 2.60/2.27/2.53 wall | 97.23/90.99/123.16 | 17.71/9.79/10.18 wall |

Panel references are **500kb synthetic**, all140, both orientations, eight workers. Sampled worker-tree peaks: CLI4 288.30MiB; library4 223.32MiB.50ms panel samples exclude the Python parent, may miss short peaks and can double-count shared pages. Library panel's original280 parser failures are retained alongside the successful reparse; its native CPU costs remain real.

The final stripped bridge is706664bytes; a matching non-PIE empty Rust executable is295288bytes, increment411376bytes. Retained MUMmer3 adds261328bytes and minimap2 adds274424bytes against their original PIE baseline. Different linking means these increments are not a controlled apples-to-apples size ranking. `libumdmummer` and delta implementation are static; OS libstdc++, libc, libm, libgcc_s/loader remain dynamic. No external executables or self-spawn are called by the MUMmer4 bridge.

On the5-Mb subset, native-reference indexes take0.80s/0.77CPU (R) and0.53s/0.51CPU (Q) to construct, each53388736bytes. Setup-inclusive alignment with `--save` takes7.44s/7.33CPU; reciprocal `--load` takes6.48s/6.35CPU. Both reproduce primary CLI4 integers/blocks. Each direction loads its own native reference's index. A common biological-Q index can serve RQ across a panel, but cannot replace QR reference indexes. No full warm-panel measurement is claimed.

Index diagnostics overlapped resumed thread checks on the same allocation: native affinity stayed within eight CPUs, but requested widths could sum to nine and compete. Those diagnostic timings are contention-sensitive. The final ledger admission rejects concurrent reservations beyond the allocation. Primary matrix/panel measurements were separate. No extra timing rerun was added to hide this deviation.

All extension work: **485.84 measured CPU-s;759.29 conservative charged seconds of7200**,370 reservations, zero outstanding reservations or cleanup-bound breaches. The addendum was frozen before alignment; original run's819.49CPU-s is separate. Matrix/panel/thread/index/runtime work shares one persistent ledger. Pair timeout90s is stricter than authorized1800s, shared across validation, both orientations and every stage; <=8workers,6GiB per worker, <=8allocated native CPUs. Reservation covers the whole deadline*CPU width plus2s cleanup. Interrupted reservations stay charged. Budget refusal stops native work; partial planned counts stay incomplete. An uninterruptible kernel process is not claimed killable within a hard deadline.

## Pins, platform and distribution

MUMmer4 **v4.0.1**, resolved commit `eb734606f2d516f42a0e0dce7a116bfb88ec1ebf`. Release archive SHA256 `bc20ae2701a0b2e323e4e515b7cfa18a0f0cb34a4ff5844b289b2de0154e3d3e`; commit archive `e950f8e19706a7a4a8507684e6e53be96ef0d5b3b50bbaaab2a65b702e206f05`.65 shared native source/header files match the resolved commit exactly. The release tarball omits top-level license/copyright files; the commit archive supplies them. Source/fixture/native-input/binary hashes and complete build argv are retained separately from public observations.

Build: GCC/G++11.5.0, C++17, `-O3 -DNDEBUG`, static library, shared/OpenMP/SWIG disabled; Rust release strip/LTO/one codegen unit, GNU Linux non-PIE link. No new external Rust dependencies. The [paper](https://journals.plos.org/ploscompbiol/article?id=10.1371/journal.pcbi.1005944) describes object-based alignment, query parallelism and reusable indexes, with equivalence except ordering. Our exact-version results qualify that claim. One long query sequence limits query-level parallelism; no bacterial-pair speedup is assumed from its large-genome benchmarks.

Linux build/runtime is demonstrated. **Mac unverified:** no suitable runner/SDK here. [v4.0.1 notes](https://github.com/mummer4/mummer/releases/tag/v4.0.1) require GCC on BSD/macOS because other compilers may give missing/nondeterministic output. Downstream macOSarm64 must build the pinned source with GCC/PIC, execute the exact matrix and difficult-input/thread/fault checks, and compare integers/complete filtered blocks to both CLI4 and the chosen profile before support is claimed. Browser work remains unverified: C++17/pthreads, filesystem/index I/O, libc RNG/stream globals, memory and exception/fatal-error behavior need adaptation; no WASM implemented.

Distribution is **not cleared by the package license alone**:

- [Artistic2.0](https://github.com/mummer4/mummer/blob/v4.0.1/LICENSE.md) applies to the package. Clause8 has a direct-interface condition; this development probe exposes the aligner/filter and future R3 exposes per-pair measurement, so do not rely solely on that clause. Proposed route: ship exact source, license/copyright/disclaimers and build instructions under clauses2/5/7; if modifications are needed, document them and use a distinct name under4(b), with source obligations under6. This is a proposed compliance route for review, not legal assurance.
- Actual included [Jellyfish parser/pool headers](https://github.com/mummer4/mummer/blob/v4.0.1/include/jellyfish/cooperative_pool2.hpp) carry **GPLv3-or-later** notices, as does [mt_skip_list/common.hpp](https://github.com/mummer4/mummer/blob/v4.0.1/include/mt_skip_list/common.hpp). They are present in the bridge's transitive native headers; GPL obligations/compatibility and any different upstream grant must be resolved before combined-binary distribution. Artistic permission alone does not establish a MIT-only combined binary.
- [Compactsufsort/divsufsort](https://github.com/mummer4/mummer/blob/v4.0.1/include/compactsufsort/divsufsort_private.h) carries the Yuta Mori permissive notice. Preserve it. CLI's [thread_pipe traits](https://github.com/mummer4/mummer/blob/v4.0.1/include/thread_pipe/traits.hpp) carry Boost1.0; preserve that notice if included. The bridge avoids CLI's thread_pipe printer, but does not avoid the GPL parser headers. Full source and component-notice hashes are retained. System runtime redistribution is a separate packaging assessment.

Our bridge/harness authorship does not relicense embedded components. No curator contact or release has occurred.

## Proposed change-control map

| Gate | Required action before proceeding |
|---|---|
| P1 | MUMmer3 keeps the frozen profile. MUMmer4 needs a separately reviewed/versioned profile naming v4.0.1 source/compiler/libc, reference-N substitution, RNG isolation, full filtering/defaults and receipts. Preserve original P1; synthetic gate agreement does not waive the method difference. |
| P4a-r3 / #25 | Propose a timestamped accepted amendment, never overwrite the design. Bind the alternative profile/binaries/reference identity at version lock; revise EN-4's reference-parser comparator and EN-3 platform checks explicitly, review existing R3/R2 record bindings and failure semantics, and restart evaluation under the new candidate version if accepted. No evaluation launched here. |
| Terobacter #8 | Remains blocked until Michael/coordinator accept a backend/profile. Then implement production bridge/isolation, PIC/vendor provenance, cache/role bindings, cancellation and complete filtered output replay. Current probe is development evidence only. |
| #30 | Verify distribution/license notices/source instructions and GPL component disposition, final build/release size, Linux/Mac GCC runtime parity, offline install and failure behavior. No release or support claim now. |
| Terobacter #2 | Share C++17/GCC, broken align_file template, global RNG/cout and GPL-parser observations. ANIm parity does not establish MLST allele semantics; no #2 files changed. |

The original evidence remains preserved; #1 stays open, #8 blocked and #24 incomplete. Return this comparison to Astra/coordinator and Michael for review before route selection.

## Reproduction

Use the [frozen addendum](mummer4-protocol.md), original generator/manifest and fresh run/build directories. Retained commands/hashes live in the private research receipt linked by the issue handoff. These templates do not authorize a new run beyond an agreed budget.

```bash
# Source: exact release archive above, with resolved-commit notices retained.
mkdir /absolute/path/to/fresh-cli-build
cd /absolute/path/to/fresh-cli-build
/absolute/path/to/mummer-4.0.1/configure \
  --disable-shared --enable-static --disable-openmp --without-swig \
  'CXXFLAGS=-O3 -DNDEBUG'
make -j8
cd /absolute/path/to/terobacter
MUMMER4_SOURCE=/absolute/path/to/mummer-4.0.1 \
MUMMER4_BUILD=/absolute/path/to/fresh-cli-build \
CARGO_TARGET_DIR=/absolute/path/to/fresh-bridge-target \
  cargo build --locked --release \
  --manifest-path investigations/anim/mummer4-probe/Cargo.toml
python investigations/anim/measure.py \
  --fixtures /absolute/path/to/preserved-fixtures \
  --output /absolute/path/to/fresh-cli4-matrix \
  --nucmer /absolute/path/to/fresh-cli-build/nucmer \
  --delta-filter /absolute/path/to/fresh-cli-build/delta-filter \
  --mummer-probe /absolute/path/to/fresh-bridge-target/release/mummer4-feasibility-probe \
  --minimap-probe /not-used --candidates cli-mummer4 \
  --budget-ledger /absolute/path/to/same-run/budget.json \
  --pair-seconds 90 --workers 8 --threads 1 --memory-kib 6291456 --sample-pairs
```

Inspect CLI comparison before library work. Every subsequent invocation uses the **same** ledger; never delete/reset it to gain budget. `compare_mummer4.py` reads retained data only; `mummer4_diagnostics.py` implements the frozen thread/index/runtime subset and does not replace prior outputs. Cargo watches native source/build trees, records native input hashes and uses a fresh target for this probe; MUMmer3 source-tree invalidation is also corrected (review N1).

Validation:20 Python metric/resource/complete-block tests pass; Clippy with warnings denied for both native probes, Rust formatting and Git whitespace checks pass. Independent Standards/Spec review found a failed-cleanup accounting defect, corrected with an overrun-and-stop fake-clock regression; both rechecks report zero outstanding findings. All15734 artifacts in the original run's manifest verify unchanged. Coordinator acceptance and Michael's backend decision remain pending.
