# Pinned measurement dependencies

skani 0.3.2 is vendored from upstream revision `22da4ae9e44b2e3e686c8e5c71fcd16cec2b9791`. Its git dependency partitions is vendored from `adf36eeaa4bb2b0042daa14cef28be36ac9be5da`. Both source trees retain their supplied licences. `UPSTREAM.json` in each directory records hashes of the original files, before local changes; source formatting follows the repository's `cargo fmt --all` check. Cargo's lockfile pins the remaining dependency graph.

The current measurement profile is `skani-0.3.2-avx2-content-order-dna-v2`. Historical reports retain `skani-0.3.2-avx2-compatible-dna-v1`. This name and the engine revision included in every adapter report. The profile reproduces the upstream x86 AVX2 DNA seeding semantics on other CPUs. It preserves the four overlapping lanes, the omitted tail k-mers, the uppercase-N pause after 21 bases, the lowercase-n encoding and lane insertion order. These details can affect fragmented assemblies near an AF threshold. They are compatibility behavior, not changes to the biological calling rules.

Local compatibility and corruption-handling changes to skani include the following files:

- `chain.rs`: the adapter entry point breaks exact orientation-score ties using sketch content. The upstream entry point retains filename ordering for historical comparisons. See the measurement change below.
- `seeding.rs`: replace the original scalar DNA seeder with portable AVX2 semantics, including marker-only operation.
- `avx2_seeding.rs`: check short inputs before slicing and use explicit wrapping arithmetic for two unused signed thresholds. Release-mode arithmetic and seed output are preserved.
- `sketch_db.rs`: check index arithmetic, bounds and payload decoding; expose index identities for marker/index validation. Corruption returns an error instead of a slice panic.

The manifests replace the partitions git dependency with its local path and omit unused upstream development dependencies and profiles. Terobacter and vendored skani both use needletail 0.7.3 for FASTA parsing; the lockfile pins the shared version. Terobacter remains one product crate; these directories contain third-party dependencies. The upstream CLI source is retained for provenance, but no skani executable is installed or invoked by Terobacter.

The adapter uses skani's chaining, screening and regression code in process, with a local Rayon pool. It avoids the upstream CLI's process-exit and warn-and-skip input paths. Inputs are assembly FASTA, including gzip; the engine's 500 bp sketching minimum is preserved. Reports retain the total input length, sketched length and every unreturned reference. Primary searches use the 15% reporting floor and 80% screen. Genomovar searches use the zero reporting floor and 80% screen, with no result-count cap. Decision values are the native writer's `f32 * 100` formatted to two decimals and stored as integer hundredths of a percent. Ties sort by emitted ANI, then reference file identity.

Direct seeding tests compare full seed maps, positions, insertion order and markers with AVX2 on supporting x86 CPUs. Frozen native vectors also exercise the portable implementation on every architecture. For the historical v1 profile, on the P2 development grid, 72 adapter runs matched 36 native runs across 12 fixture/component pairs, with identical emitted values and row presence. Adapter JSON was byte-identical across automatic/forced-portable seeding and 1, 2 and 8 threads. Commands, input hashes, frozen-result bindings and logs are retained in the research handoff for issue #6, rather than in this repository.

Actual macOS execution, full primary/genomovar evaluation and release claims remain separate gates. Forced portable execution on Linux establishes implementation parity on these development inputs.

Third-party redistribution notices are in [THIRDPARTY.json](../THIRDPARTY.json) and [THIRD-PARTY-NOTICES.txt](../THIRD-PARTY-NOTICES.txt). [THIRD-PARTY-LICENSES.md](../THIRD-PARTY-LICENSES.md) describes regeneration and redistribution.

## Warning maintenance

Local partitions maintenance removes the obsolete `cargo-clippy` feature check, uses the upstream rank-bit formula instead of unsupported pointer-width cfg values, removes one unused `mut`, and spells elided iterator lifetimes consistently. The compact formula preserves the upstream rank widths on 16-, 32- and 64-bit targets. Default and compact union/find and set-iteration tests check behavior.

Local skani maintenance spells the `Cow` lifetime and removes a discarded interval copy. Chaining continues to push the original interval, including its original overlap value. A small-overlap regression guards this behavior. Those warning-maintenance changes did not change the measurement profile, algorithms or dependency versions. Original file hashes in both `UPSTREAM.json` files remain unchanged; they identify upstream bytes rather than the patched sources.

Vendored source bytes included in the Rust source identity. This maintenance produces a new source/native build candidate; frozen evidence and platform bundles must retain their original identities. Numerical compatibility and binary/provenance equality are separate checks.

## Filename-independent orientation

The v2 adapter preserves the upstream orientation scores. Only when they are exactly equal does it compare sketch content, in this order: total sequence length, the ordered contig lengths, k, c, marker density, sorted marker seeds, and sorted seed keys with their sorted decoded positions. It switches roles when the query key is greater than the reference key; identical keys retain input roles. File paths, contig headers, hash-map iteration order and packed-storage layout are excluded. Contig order remains part of the measurement input; this change does not promise invariance to rearranging contigs.

This rule needs no new sketch format or database rebuild. It is a Pterobacter measurement change, not a claim of upstream numerical parity: tied pairs can have different ANI or aligned fractions from the historical filename-selected orientation. The profile identifier distinguishes those reports, and the build identity covers the changed vendored source. Existing saved reports and validation results are not rewritten. Scientific re-evaluation and release acceptance are separate gates.

Synthetic tests exercise a known orientation-sensitive pair, query and reference renaming, gzip/bzip2/xz/zstd inputs and reciprocal measurements, portable/native seeding, thread counts, identical sketches, unequal scores and canonical content ordering. Historical saved-evidence fixtures continue to carry their original profile.
