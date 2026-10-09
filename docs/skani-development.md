# Embedded skani development check

Build the feature-gated development binary:

```sh
cargo build --release --locked --features dev --bin terobacter-dev
terobacter-dev skani-search --database DB --query assembly.fa --threads 2
```

`DB` is an authenticated consolidated skani 0.3.2 DNA database component containing `markers.bin`, `index.db` and `sketches.db`. Keep these files immutable while the database is open. Reference-package authentication is provided by the package loader; this measurement entry point checks the database format and internal bindings.

Add `--portable` to force the portable seeder on an AVX2 CPU. Add `--genomovar` for the zero reporting floor and uncapped results used by the separate genomovar component. Both modes retain the 80% screen. The JSON contains the engine profile, revision, search settings, two-decimal ANI/AF rows, input/sketched lengths and reference files without an estimate. It performs no assignment.

The installed public commands keep their existing interface. `skani-search` is available only in the development binary. See [the fork notes](../vendor/README.md) for the compatibility contract and validation limits.
