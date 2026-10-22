# Install Pterobacter

Install version 0.1.0 with Cargo, download a precompiled binary, or use the Docker image. Bioconda and Homebrew formulae are not currently provided.

## Cargo

Install Rust and GNU C++ with C++17 support, then run:

```sh
cargo install pterobacter --version 0.1.0 --locked
ptero --version
```

On macOS, install the Xcode command-line tools and Homebrew GCC first, as described below.

## Precompiled binaries

Download the archive for your platform and `SHA256SUMS` from the [0.1.0 release](https://github.com/mbhall88/pterobacter/releases/tag/0.1.0). Linux x86_64 binaries require glibc and the GNU C++ runtime. Apple Silicon macOS binaries require Homebrew runtimes: `brew install gcc xz`. Each archive includes its measured runtime-library requirements, licences and checksums. Linux ARM and Windows are untested.

For example, on Linux x86_64:

```sh
curl --fail --location --remote-name \
  https://github.com/mbhall88/pterobacter/releases/download/0.1.0/pterobacter-0.1.0-x86_64-unknown-linux-gnu.tar.gz
curl --fail --location --remote-name \
  https://github.com/mbhall88/pterobacter/releases/download/0.1.0/SHA256SUMS
grep ' pterobacter-0.1.0-x86_64-unknown-linux-gnu.tar.gz$' SHA256SUMS | sha256sum --check -
tar -xzf pterobacter-0.1.0-x86_64-unknown-linux-gnu.tar.gz
./pterobacter-0.1.0-x86_64-unknown-linux-gnu/bin/ptero --version
```

For Apple Silicon, use the `aarch64-apple-darwin` archive and `shasum -a 256 --check` for checksum verification. Add the extracted `bin` directory to your `PATH` or invoke its commands by their paths. Complete corresponding source is provided beside the binaries.

## Linux source build

Install Rust and GNU C++ with C++17 support, then obtain the source:

```sh
git clone https://github.com/mbhall88/pterobacter.git
cd pterobacter
cargo install --path . --locked
ptero --version
pterobacter --version
```

Both installed names run the same program. Use `pterobacter` if another installed tool already uses `ptero`. Cargo's installation directory must be on your `PATH`. It is usually `~/.cargo/bin`.

To build without installing, use:

```sh
cargo build --release --locked
./target/release/ptero --version
```

For a debug build, use `cargo build --locked` and `./target/debug/ptero`. These local paths apply to `cargo build`; `cargo install` makes the commands available through Cargo's bin directory.

The measured Linux build used Rust 1.95.0 and GCC 11.5.0 on x86_64 Red Hat Enterprise Linux 9.8. The binary links system C/C++ libraries; the inspected build requires glibc symbols through 2.34. These describe the tested build, not a minimum Rust version or compatibility guarantee for every Linux system. The source includes Rust and embedded C/C++.

## macOS source build

Install the Xcode command-line tools and Homebrew GNU GCC. Native builds discover installed Homebrew GNU C++ automatically; an explicit Apple clang override is refused by the pinned native build:

```sh
xcode-select --install  # skip if already installed
brew install gcc
cargo install --path . --locked
```

Discovery checks the installed gcc prefix and supports Apple Silicon and Intel Homebrew locations. Explicit compiler overrides retain cc-rs precedence; empty or whitespace-only values fail with a variable-specific error. Unset the variable to request discovery. Keep the compiler's GNU C++ runtime available. macOS CI build/test smoke and the fixed scientific parity panel on an Apple Silicon Mac have passed. Linux ARM and Windows are untested. The native build currently targets Linux and macOS.

## Build offline

First provision Rust, GNU C++, the source and its locked Cargo dependencies through a cache or vendor directory. From that provisioned source tree:

```sh
CARGO_NET_OFFLINE=true cargo install --path . --locked --offline
```

An uncached machine still needs to obtain those prerequisites. At runtime the program embeds skani, MUMmer4 and the MLST matcher; no external analysis executable is required.

## Next: reference data

Classification needs reference data separately from the executable. Run `ptero database install 2026-10-15`, then follow [database setup](databases.md) and [run an assembly](running.md). Retain the software version, database version and input identity with your analysis record.

For test commands and the optional development binary, see [Contributing](../CONTRIBUTING.md). [Release preparation](release-preparation.md) records the checked platforms and publication steps.

## Docker

Use `ghcr.io/mbhall88/pterobacter:0.1.0`, or build the Dockerfile from source. See [containers](containers.md) for database and assembly mounts.
