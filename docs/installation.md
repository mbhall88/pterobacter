# Install Pterobacter

Install with Bioconda or Cargo, download a precompiled binary, or use the Docker image.

## Bioconda

[![Bioconda version](https://img.shields.io/conda/vn/bioconda/pterobacter)](https://anaconda.org/bioconda/pterobacter) [![Bioconda downloads](https://img.shields.io/conda/dn/bioconda/pterobacter)](https://anaconda.org/bioconda/pterobacter)

```sh
conda install -c conda-forge -c bioconda pterobacter
ptero --version
```

## Cargo

[![Crates.io version](https://img.shields.io/crates/v/pterobacter)](https://crates.io/crates/pterobacter) [![Crates.io downloads](https://img.shields.io/crates/d/pterobacter)](https://crates.io/crates/pterobacter)

Install Rust and GNU C++ with C++17 support, then run:

```sh
cargo install pterobacter --locked
ptero --version
```

On macOS, install the Xcode command-line tools and Homebrew GCC first, as described below.

## Precompiled binaries

[![GitHub release](https://img.shields.io/github/v/release/mbhall88/pterobacter)](https://github.com/mbhall88/pterobacter/releases/latest) [![Release downloads](https://img.shields.io/github/downloads/mbhall88/pterobacter/total)](https://github.com/mbhall88/pterobacter/releases/latest)

The [latest release](https://github.com/mbhall88/pterobacter/releases/latest) provides these archives:

| Platform | Archive suffix |
| --- | --- |
| Linux x86_64 | `x86_64-unknown-linux-gnu.tar.gz` |
| macOS Apple Silicon | `aarch64-apple-darwin.tar.gz` |

Each archive contains both commands, licences and runtime requirements. Rust is not needed to use these binaries. The release page also provides checksums and complete corresponding source.

### Linux x86_64

The released binary needs glibc 2.34 or newer and a GNU C++ runtime providing `GLIBCXX_3.4.29` or newer. If your system is older, use the container or check the source-build requirements below. These commands find the latest release, then download and verify its archive:

```sh
mkdir pterobacter-download
cd pterobacter-download
release_url=$(curl --fail --silent --show-error --location \
  --output /dev/null --write-out '%{url_effective}' \
  https://github.com/mbhall88/pterobacter/releases/latest)
version=${release_url##*/}
archive="pterobacter-${version}-x86_64-unknown-linux-gnu.tar.gz"
base="https://github.com/mbhall88/pterobacter/releases/download/${version}"
curl --fail --location --remote-name "$base/$archive"
curl --fail --location --remote-name "$base/SHA256SUMS"
awk -v name="$archive" '$2 == name' SHA256SUMS | sha256sum --check -
tar -xzf "$archive"
export PATH="$PWD/${archive%.tar.gz}/bin:$PATH"
ptero --version
```

### macOS Apple Silicon

Install the Homebrew runtime libraries, then download and verify the latest release:

```sh
brew install gcc xz
mkdir pterobacter-download
cd pterobacter-download
release_url=$(curl --fail --silent --show-error --location \
  --output /dev/null --write-out '%{url_effective}' \
  https://github.com/mbhall88/pterobacter/releases/latest)
version=${release_url##*/}
archive="pterobacter-${version}-aarch64-apple-darwin.tar.gz"
base="https://github.com/mbhall88/pterobacter/releases/download/${version}"
curl --fail --location --remote-name "$base/$archive"
curl --fail --location --remote-name "$base/SHA256SUMS"
awk -v name="$archive" '$2 == name' SHA256SUMS | shasum -a 256 --check -
tar -xzf "$archive"
export PATH="$PWD/${archive%.tar.gz}/bin:$PATH"
ptero --version
```

The `export PATH` command applies to the current terminal. For future terminals, add the extracted `bin` directory's absolute path to your shell configuration, or invoke `bin/ptero` by its full path. Keep the extracted directory in a permanent location before adding it to your configuration.

## Linux source build

Install Rust and GNU C++ with C++17 support, then obtain the source. Linux source builds require glibc 2.28 or newer for the database installer:

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


## macOS source build

Install the Xcode command-line tools and Homebrew GNU GCC. Native builds discover installed Homebrew GNU C++ automatically; an explicit Apple clang override is refused by the pinned native build:

```sh
xcode-select --install  # skip if already installed
brew install gcc
git clone https://github.com/mbhall88/pterobacter.git
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

[![Container version](https://ghcr-badge.egpl.dev/mbhall88/pterobacter/latest_tag?label=GHCR)](https://github.com/mbhall88/pterobacter/pkgs/container/pterobacter) [![Container downloads](https://ghcr-badge.elias.eu.org/shield/mbhall88/pterobacter/pterobacter)](https://github.com/mbhall88/pterobacter/pkgs/container/pterobacter)

Pull the image from [GitHub Container Registry](https://github.com/mbhall88/pterobacter/pkgs/container/pterobacter), or build the Dockerfile from source. See [containers](containers.md) for download commands and database/assembly mounts.

### Packaging with a macOS GNU/libc++ toolchain

The default GNU build links libstdc++; native Homebrew discovery is unchanged.
For a macOS packaging toolchain that supplies GNU C++ configured for libc++, set
an explicit compiler and `PTERO_CXX_STDLIB=c++`. For example, after activating
Conda's native compiler environment:

```sh
PTERO_CXX_STDLIB=c++ cargo build --release --locked
```

The environment must already supply `CXX` or a target-specific GNU C++ override,
libc++ headers, and linker search paths. This mode skips Homebrew runtime
lookup and selects the matching libc++ string layout on Apple Silicon.
An active Conda environment alone does not select this mode. Empty or unsupported
runtime values fail the build; GNU C++ remains required.
