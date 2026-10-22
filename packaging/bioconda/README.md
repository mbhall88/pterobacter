# Bioconda recipe

This is a submission candidate for Pterobacter 0.1.0, not an available Bioconda package. Copy `pterobacter/` into `recipes/pterobacter/` in a checkout of [bioconda-recipes](https://github.com/bioconda/bioconda-recipes).

The recipe follows the [Rasusa](https://github.com/bioconda/bioconda-recipes/blob/master/recipes/rasusa/meta.yaml) and [LRGE](https://github.com/bioconda/bioconda-recipes/tree/master/recipes/lrge) recipes and the [Bioconda Rust guidelines](https://bioconda.github.io/contributor/guidelines.html#rust). It pins the released source archive and lockfile, installs `ptero` and `pterobacter`, retains Rust and embedded native licence notices, and runs the installed-program test with synthetic inputs, including native ANIm. The combined executable is GPL-3.0-or-later; our own source remains MIT.

The candidate targets four native platforms:

| Platform | Conda subdirectory | Validation |
| --- | --- | --- |
| Linux x86_64 | `linux-64` | Local package build and installed ANIm smoke test passed before platform expansion; CI rechecks the expanded recipe. |
| Linux ARM64 | `linux-aarch64` | Native build and runtime tests required. |
| macOS Intel | `osx-64` | Native build and runtime tests required. |
| macOS Apple Silicon | `osx-arm64` | Native build and runtime tests required. |

Linux requires glibc 2.28 or newer. Database activation calls the `renameat2` libc wrapper, which is absent from Bioconda's default 2.17 sysroot. The macOS candidate targets macOS 11 or newer.

The embedded backend requires GNU C++. On macOS, the recipe selects Conda's GNU 16 compiler and applies `conda-macos-libcxx.patch` to `build.rs`: Conda's Darwin compiler uses `libc++`, whereas the upstream Homebrew build uses `libstdc++`. The patch selects Conda's runtime and removes Homebrew runtime discovery; it changes no alignment or classification code. It applies only to the macOS package and is included in that package's source recipe. Native engine build hashes record the patched build script. Linux builds use the unpatched release source. Neither platform uses Homebrew in its Conda build.

`.github/workflows/bioconda.yml` builds on native runners for all four platforms, retaining logs and packages. Each build must pass the installed-program test, including embedded ANIm, and the relocated Conda package tests. The recipe refuses cross-compilation rather than skipping these runtime checks. Dependency resolution alone is not a platform-support result. Bioconda's own CI remains required before channel submission.

The reference database is downloaded separately with `ptero database install 2026-10-15`; it is not included or downloaded during package installation or tests. The existing [complete corresponding source archive](https://github.com/mbhall88/pterobacter/releases/download/0.1.0/pterobacter-0.1.0-corresponding-source.tar.gz) accompanies the upstream release.

Run Bioconda's lint/build workflow before submission. A direct local check with conda-build is:

```sh
conda-build --override-channels -c conda-forge -c bioconda packaging/bioconda/pterobacter
```

Bioconda CI uses its own compiler pins and build environment; a local build does not replace that check. Do not advertise `conda install pterobacter` until the recipe has been accepted and a package is available in the channel.
