# Embedded Rust implementations

The reviewed skani and partitions implementations are compiled as modules of the single Pterobacter crate. Cargo cannot publish a package whose runtime dependencies exist only as local paths. Their original manifests are retained as `Cargo.toml.upstream`; those filenames avoid creating nested packages that Cargo would omit from the archive.

The adaptation changes Rust namespaces, macro visibility, equivalent single-condition cfg syntax, and doctest paths. The upstream skani command parser is not compiled into Pterobacter; sketch database I/O remains compiled. The partitions rayon/proptest implementations remain enabled as they were under its original default features. `embedded-partitions-compact` exercises its alternative representation in tests.

`scripts/check_embedded_sources.py` checks all source files against baseline commit `2ec46e32c24d8f50689226cffaddca0caa915b3d`, allowing only these recorded adaptations. The baseline includes original and canonical hashes. Algorithm changes must receive separate review and must not be hidden by extending that normalizer.

Upstream licences, attribution and the previously reviewed portability changes remain in each vendor directory. Supplemental licence generation includes both implementations even though they are no longer separate Cargo dependencies.

The pinned cargo-shear 1.14 checker does not normalize `../vendor` module paths. Cargo metadata lists exceptions only for imports used by those compiled modules and their source paths. The source audit, native builds and tests remain required; remove the exceptions when the checker follows these paths.
