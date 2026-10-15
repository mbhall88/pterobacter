# Install Pterobacter

Build from the private source if you have repository access. Distribution is on hold during repository review; no public binary, crates.io package, Bioconda package or Homebrew formula is currently offered.

## Linux source build

Install Rust and GNU C++ with C++17 support, then obtain the source:

```sh
git clone git@github.com:mbhall88/pterobacter.git
cd pterobacter
cargo install --path . --locked
ptero --version
pterobacter --version
```

Both installed names run the same program. Use `pterobacter` if another installed tool already uses `ptero`. Cargo's installation directory must be on your `PATH`.

The measured Linux build used Rust 1.95.0 and GCC 11.5.0 on x86_64 Red Hat Enterprise Linux 9.8. The binary links system C/C++ libraries; the inspected build requires glibc symbols through 2.34. These describe the tested build, not a minimum Rust version or compatibility guarantee for every Linux system. The source includes Rust and embedded C/C++.

## macOS source build

Install the Xcode command-line tools and Homebrew GNU GCC. Native builds discover installed Homebrew GNU C++ automatically; an explicit Apple clang override is refused by the pinned native build:

```sh
xcode-select --install  # skip if already installed
brew install gcc
cargo install --path . --locked
```

Discovery checks the installed gcc prefix and supports Apple Silicon and Intel Homebrew locations. Explicit compiler overrides retain cc-rs precedence; empty or whitespace-only values fail with a variable-specific error. Unset the variable to request discovery. Keep the compiler's GNU C++ runtime available. macOS CI build/test smoke passes; scientific parity on the final arm64 panel is still pending. Linux ARM and Windows are untested. The native build currently targets Linux and macOS.

## Build offline

First provision Rust, GNU C++, the source and its locked Cargo dependencies through a cache or vendor directory. From that provisioned source tree:

```sh
CARGO_NET_OFFLINE=true cargo install --path . --locked --offline
```

An uncached machine still needs to obtain those prerequisites. At runtime the program embeds skani, MUMmer4 and the historical MLST matcher; no external analysis executable is required.

## Next: reference data

Classification needs reference data separately from the executable. Obtain the directory and its trusted digest through the private handoff, then follow [reference setup](reference-setup.md) and [run an assembly](running.md). The current source version is `0.0.0`; retain the supplied source commit and binary identity with your analysis record.

For test commands and the optional development binary, see [Contributing](../CONTRIBUTING.md). [Measured package and platform details](release-preparation.md) remain available for packaging review.
