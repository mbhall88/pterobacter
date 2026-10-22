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

| Platform | Archive |
| --- | --- |
| Linux x86_64 | [Download](https://github.com/mbhall88/pterobacter/releases/download/0.1.0/pterobacter-0.1.0-x86_64-unknown-linux-gnu.tar.gz) |
| macOS Apple Silicon | [Download](https://github.com/mbhall88/pterobacter/releases/download/0.1.0/pterobacter-0.1.0-aarch64-apple-darwin.tar.gz) |

Each archive contains both commands, licences and runtime requirements. Rust is not needed to use these binaries. Intel macOS, Linux ARM and Windows binaries are not provided for this release. The [release page](https://github.com/mbhall88/pterobacter/releases/tag/0.1.0) also provides checksums and complete corresponding source.

### Linux x86_64

The released binary needs glibc 2.34 or newer and a GNU C++ runtime providing `GLIBCXX_3.4.29` or newer. If your system is older, use the container or check the source-build requirements below. Use a new download directory:

```sh
mkdir pterobacter-0.1.0-download
cd pterobacter-0.1.0-download
curl --fail --location --remote-name \
  https://github.com/mbhall88/pterobacter/releases/download/0.1.0/pterobacter-0.1.0-x86_64-unknown-linux-gnu.tar.gz
curl --fail --location --remote-name \
  https://github.com/mbhall88/pterobacter/releases/download/0.1.0/SHA256SUMS
grep ' pterobacter-0.1.0-x86_64-unknown-linux-gnu.tar.gz$' SHA256SUMS | sha256sum --check -
tar -xzf pterobacter-0.1.0-x86_64-unknown-linux-gnu.tar.gz
export PATH="$PWD/pterobacter-0.1.0-x86_64-unknown-linux-gnu/bin:$PATH"
ptero --version
```

### macOS Apple Silicon

Install the required Homebrew runtime libraries, then download and verify the archive:

```sh
brew install gcc xz
mkdir pterobacter-0.1.0-download
cd pterobacter-0.1.0-download
curl --fail --location --remote-name \
  https://github.com/mbhall88/pterobacter/releases/download/0.1.0/pterobacter-0.1.0-aarch64-apple-darwin.tar.gz
curl --fail --location --remote-name \
  https://github.com/mbhall88/pterobacter/releases/download/0.1.0/SHA256SUMS
grep ' pterobacter-0.1.0-aarch64-apple-darwin.tar.gz$' SHA256SUMS | shasum -a 256 --check -
tar -xzf pterobacter-0.1.0-aarch64-apple-darwin.tar.gz
export PATH="$PWD/pterobacter-0.1.0-aarch64-apple-darwin/bin:$PATH"
ptero --version
```

The `export PATH` command applies to the current terminal. For future terminals, add the extracted `bin` directory's absolute path to your shell configuration, or invoke `bin/ptero` by its full path. Keep the extracted directory in a permanent location before adding it to your configuration.

## Bioconda

A [Bioconda recipe](https://github.com/mbhall88/pterobacter/tree/main/packaging/bioconda) is being prepared for Linux x86_64 and ARM64, and macOS Intel and Apple Silicon. It has not yet been accepted into the channel. Use Cargo, a release binary or Docker until a package is available.

## Linux source build

Install Rust and GNU C++ with C++17 support, then obtain the source. Linux source builds require glibc 2.28 or newer for the database installer:

```sh
git clone --branch 0.1.0 --depth 1 https://github.com/mbhall88/pterobacter.git
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
git clone --branch 0.1.0 --depth 1 https://github.com/mbhall88/pterobacter.git
cd pterobacter
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
