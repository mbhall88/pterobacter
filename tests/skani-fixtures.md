# skani development fixtures

These tests use synthetic sequences. They export no reference assemblies or research records. The P2 development grid is checked in the research workspace, with query/database hashes and retained native outputs bound in the issue #6 handoff.

`skani_seeding.rs` generates DNA with a fixed xorshift seed, then inserts N/n at the four-lane boundaries. The complete position maps and markers are compared directly with the pinned AVX2 implementation. Golden digests were obtained from that native seeder at upstream revision `22da4ae9e44b2e3e686c8e5c71fcd16cec2b9791`. Seed keys are sorted, each key is followed by its packed positions in insertion order and a `u64::MAX` separator, then sorted markers are appended. FNV-1a over their little-endian bytes gives the stored digest. Lengths 500 through 503 have the same digest because AVX2 intentionally drops the final remainder. This test runs on every architecture.

`skani_adapter.rs` builds two identical 200,003 bp synthetic references with different identifiers. It checks tie ordering, emitted values, thread/path parity and concurrent calls. A query with 20 kb of shared sequence checks that the zero-floor genomovar search retains rows excluded by the primary 15% reporting floor. Invalid FASTA/FASTQ, short contigs and corrupt database fixtures check error and no-estimate states.

The P2 native comparator is skani 0.3.2, binary SHA-256 `b3000043feb4ce8a5200b3716bae155f34f73e2a123b8b055d574a7897a14151`. The development-grid replay reproduced its retained P2 values; these tests do not claim new biological accuracy or macOS execution.
