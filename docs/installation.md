# Installation and provisioned use

This is a private development candidate. A published binary, crates.io package, Bioconda package and Homebrew formula are not available from this preparation. Build from a reviewed source export or the private repository. The two installed public commands are `tero` and `terobacter`; they implement the same program and take assembly paths directly, without subcommands.

## Build from source

The measured Linux build used Rust 1.95.0 and GCC 11.5.0. Rust and GNU C++ with C++17 support are build prerequisites. `just` is optional; it runs the repository checks but is not needed to install.

```sh
cargo install --path . --locked
tero --version
terobacter --version
```

The development candidate reports `terobacter 0.0.0`. Identify it by the exact source commit and binary SHA-256, not by that placeholder version alone. The private evaluated baseline is `834f4c288e9c163f316f3668f0219004d0adc6da`.

On macOS, install the Xcode command-line tools and Homebrew GNU GCC before building. Set `CXX` to the installed GNU compiler, for example:

```sh
export CXX=/opt/homebrew/bin/g++-15  # use your installed Homebrew GCC version
cargo install --path . --locked
```

The compiler and its GNU C++ runtime are required. Apple clang is refused by the pinned build. macOS CI build/test smoke passed for the evaluated candidate, but the final arm64 scientific parity panel remains pending. Linux ARM and Windows are untested; the native process-containment build currently targets Linux and macOS. See [measured support](release-preparation.md).

## Offline build and analysis

A source build is offline only after its locked dependencies have been cached or vendored. From a provisioned source tree:

```sh
CARGO_NET_OFFLINE=true cargo install --path . --locked --offline
```

The independent installation check used a fresh target/install directory and cached registry sources. It does not claim a fresh machine can build without first obtaining Rust, GNU C++, and those dependencies. At runtime the measured Linux binary links system C/C++ libraries. It embeds skani, MUMmer4 and the historical MLST matcher, so it requires no external analysis executable. It contains Rust and C/C++ code.

Classification needs a separately provisioned reference directory and the trusted SHA-256 of its `portable.json`. Copying a checksum from the same untrusted download does not authenticate the package. The candidate data remains private; no public download URL is promised here.

```sh
tero --reference-package reference --expected-sha256 "$REFERENCE_SHA256" \
  --threads 2 --report-dir isolate-report isolate.fa

tero --reference-package reference --expected-sha256 "$REFERENCE_SHA256" \
  --jobs 2 --threads 2 --output batch.json isolate_1.fa isolate_2.fa
```

All supplied records remain in the query, including drafts and plasmids. Gzip, bzip2, xz and zstd FASTA inputs retain supplied-byte provenance. Reports distinguish total bases from bases in records long enough to sketch. A provisioned analysis makes no network request. The output paths must be new and outside the reference/input directories. In a batch, inspect each record's execution status; a failure is not an unresolved biological result. Jobs multiply the search-thread allocation.

The default policy uses skani. The candidate v2 policy also needs the authenticated primary FASTA component when a query triggers ANIm:

```sh
tero --reference-package reference --expected-sha256 "$REFERENCE_SHA256" \
  --policy conservative-assembly-anim-v2 \
  --anim-reference-package reference-fastas --expected-anim-sha256 "$ANIM_SHA256" \
  --anim-output new-anim-evidence --report-dir new-v2-report isolate.fa
```

Optional historical MLST adds `--mlst-package mlst --mlst-expected-sha256 "$MLST_SHA256"`. Its 2026 retrieval covers submissions through 31 December 2024; it is not a byte snapshot of that date or a claim of complete historical coverage. MLST never changes the taxonomic call.

The earlier Python research CLI and `legacy_cli` are historical interfaces outside this tool distribution. Percentage fields use declared percent units: `0.8` means 0.8%, and `80` means 80%. Withheld assignment fields remain null while nearest-reference evidence is retained separately. The old AF/withheld-label defects are not represented as fixes to the historical source. Release checks discover only this Rust source export; they do not import research browser modules or modify frozen figures.
