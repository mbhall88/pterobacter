# P1-ANIM-MUMMER4-r1

Issue #8 implements the method approved after issues #1 and #3. This amendment adds a maintained MUMmer4 profile beside the frozen MUMmer3 comparator. It does not replace P1, change the calling policy, or authorize biological validation or release.

## Method identity

`reciprocal-mummer4-4.0.1-preserve-n-mum-1-span-v1` uses MUMmer4 v4.0.1 at `eb734606f2d516f42a0e0dce7a116bfb88ec1ebf`, with the five-line patch retained in `investigations/anim/mummer4-maintenance.patch`. It selects unique anchors in both inputs with `Options::mum`, one native query thread, default extension parameters and `flag1to1(-1,100)` filtering. Each orientation starts in a fresh process and seeds libc with `srand(1)`. GCC, compiler flags, native sources and target are part of the build digest. The Rust source and dependency lockfile have a separate digest; receipts also retain the producing executable digest.

Reference Ns remain literal characters. Identical ambiguous characters retain MUMmer3's identity behavior; they are not masked from the numerator or denominator. The known long-N alignment limitation remains. Different compilers, targets or libc RNG implementations require their own parity evidence.

## Integers and roles

For each filtered block, sum both inclusive spans into D and the fifth header integer into E. Identity is the exact rational percentage `100 * (D - 2E) / D`. Coverage is the inclusive interval union per contig divided by the whole input length, including Ns and contigs with no alignment. Reverse coordinates are normalized for coverage. Offset lines describe a block; they never create additional blocks.

All sums and cross products are checked. Identity uses a strict greater-than comparison; coverage uses an inclusive greater-than-or-equal comparison. Formatting percentages cannot decide a gate.

QR makes the biological reference the native reference. RQ swaps the native inputs. In both result records, `query_covered`, `query_length`, `reference_covered` and `reference_length` name the biological roles. `native_reference_id` and `native_query_id` retain the native axes explicitly. A valid header-only empty filtered delta produces `no_alignment`, D=E=0, zero coverage and undefined identity. A failed or truncated operation produces an incomplete pair, never a biological negative.

## Inputs and receipts

`terobacter-dev anim-pair` copies and validates whole FASTAs before native execution. IDs default to byte hashes; callers can supply package IDs, reference component and concept IDs. The command checks expected hashes against the bytes it copied. It also writes and hashes native FASTAs with LF line endings and sequence-line whitespace removed exactly as validation permits. Record IDs and every nucleotide stay in their original order; descriptions are omitted. Both native axes use these derived files so CRLF wrapping cannot remove anchors or alter lengths. Original byte snapshots remain unchanged. These labels bind an operation; package authentication belongs to the reference-package integration.

A fresh output directory is required. `request.json`, input snapshots, QR/RQ raw and filtered delta files, worker stdout/stderr, exit/signal status, measured wall time, successful-worker CPU/RSS, and artifact hashes remain beside `result.json`. Failed input bytes are retained when readable within the input limit. Every started native process has a receipt; orientations skipped after failure are `not_run`. Worker failure must have both a successful process exit and a valid successful native completion record to become complete.

`verify_pair` requires expected input hashes, the current source/build profile, retained snapshots and artifact hashes, correct native axes, and recomputed exact metrics. Executable hashes remain producer provenance; the public aliases and development binary can share a source/build profile while their executable bytes differ. This is a reuse validator, not a cache scheduler.

## Containment and limits

Workers are processes of the same executable and require no external alignment or filtering command. Each worker has a separate libc RNG state. The coordinator handles SIGINT/SIGTERM, kills the worker process group and retains an incomplete record. Linux also installs a parent-death signal; a portable watchdog checks parent identity and the deadline. Parent SIGKILL cannot leave a native worker running indefinitely, although the coordinator cannot write a final receipt after SIGKILL.

The default shared reciprocal deadline is 1800 seconds and the worker address-space ceiling is 6144 MiB. Callers may lower these to positive deadlines and 64–6144 MiB. Each input is limited to 256 MiB of FASTA bytes. Core dumps are disabled; worker CPU time is bounded by its remaining wall deadline. Resource limits are engineering ceilings, not evidence that a method meets the biological performance gate.

Linux uses GNU C++. macOS CI installs GNU C++ and links its runtime; native macOS parity and distribution of that runtime remain acceptance gates until evidence is recorded. Full-panel and difficult-case parity receipts belong to the fresh research run, not this source contract.
