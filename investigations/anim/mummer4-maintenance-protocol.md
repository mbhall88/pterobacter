# MUMmer4 maintenance diagnostic

Codex GPT-6 (exact backend identifier unavailable), 9 October 2026. Michael
requested further MUMmer4 exploration because integration and maintenance may
outweigh strict upstream compatibility. This is a diagnostic candidate, not a
backend selection, production implementation, or amendment to frozen P1/P4a.

Preserve both previous research runs. Copy the previous MUMmer4 ledger into
the fresh run, bind its hash and carry its charges forward within the original
7,200-second limit. Never run concurrently with another ledger descendant.
Use the existing harness: 90-second reciprocal deadline, 6-GiB process-tree
ceiling, at most eight allocated CPUs, explicit affinity and persistent admission.
Build work is logged separately. Stop on resource refusal, unsafe native failure,
or two focused build repairs. No panel or index timing rerun, biological data,
minimap2 rerun, threshold tuning, or upstream contact.

Before building or aligning, record protocol/tool/source/input hashes and three
small upstream changes, each visible in the retained patch:

1. Remove reference `N` substitution in `sequence_info`. Preserve input bases;
   this deliberately changes upstream semantics and is not claimed as upstream
   MUMmer4 equivalence.
2. Make `DeltaGraph::outputDelta` honor its output stream, removing global
   `cout` redirection from the exploratory bridge.
3. Fix `align_file`'s trampoline template parameters so the bridge uses that
   public API. Use PIC native objects. Keep the existing one-to-one filter and
   its serialized/reseeded libc RNG; do not silently change tie selection.

Run modified CLI and modified library on exactly the original 20 valid matrix
pairs plus four invalid cases. Retained MUMmer3 and upstream MUMmer4 outputs are
read-only controls. Compare status, exact D/E/covered-base/length integers,
continuous identity/AF, gates, and complete filtered/unfiltered delta blocks.
Keep failed and incomplete measurements. The library pass must use its actual
output files, fixing the earlier primary adapter-validation gap.

One additional diagnostic uses the original ambiguity query FASTA against itself
(no new sequence bytes) in both directions. Run retained CLI3, retained CLI4 and
modified CLI4 to distinguish N–N identity from positive-score similarity. This
is the stated reason for a new comparator execution. Report delta columns five
and six separately; the P1 metric continues to use column five.

For in-process concurrency, use the modified bridge on fragmented,
repeat_reference, ambiguous and genome_5mb, both orientations, two repetitions:
16 calls in one process, first with one worker and then eight workers, one native
query thread per call. Each complete batch reserves a 90-second deadline times
its CPU width with the same ledger and memory cap. Compare every complete block
and integer record to the modified-library matrix. Record batch wall/CPU/RSS;
these small single measurements are not a panel/laptop speedup claim. This tests
shared-process objects, not merely concurrent executable invocations. Global
filter RNG locking remains a host-integration limitation.

Return the patch size, numerical/concurrency evidence, remaining maintenance
burden and method/platform/licensing questions for review. No macOS/WASM claim,
merge, release, #25 launch, scientific-label change or automatic selection.
