# Pterobacter

<img src="docs/assets/pterobacter.png" alt="Pterobacter mascot" width="180" align="right">

Pronounced with a silent P, like pterodactyl.

[![CI](https://github.com/mbhall88/pterobacter/actions/workflows/ci.yml/badge.svg)](https://github.com/mbhall88/pterobacter/actions/workflows/ci.yml) [![User guide](https://img.shields.io/badge/docs-user_guide-236b73)](https://mbhall88.github.io/pterobacter/)

Classify *Enterobacter* isolate assemblies by species and subspecies, with sequence typing (MLST), genomovar matches where supported, and warnings for uncertain calls. Supply a whole assembly in FASTA format, including plasmids and draft contigs.

## Install

### Download a binary

| Platform | Download |
| --- | --- |
| Linux x86_64 | [pterobacter-0.1.0-x86_64-unknown-linux-gnu.tar.gz](https://github.com/mbhall88/pterobacter/releases/download/0.1.0/pterobacter-0.1.0-x86_64-unknown-linux-gnu.tar.gz) |
| macOS Apple Silicon | [pterobacter-0.1.0-aarch64-apple-darwin.tar.gz](https://github.com/mbhall88/pterobacter/releases/download/0.1.0/pterobacter-0.1.0-aarch64-apple-darwin.tar.gz) |

Download, verify and unpack the archive using the [Linux instructions](https://mbhall88.github.io/pterobacter/installation.html#linux-x86_64) or [macOS instructions](https://mbhall88.github.io/pterobacter/installation.html#macos-apple-silicon). Linux needs glibc and the GNU C++ runtime; macOS needs `brew install gcc xz`. [Checksums](https://github.com/mbhall88/pterobacter/releases/download/0.1.0/SHA256SUMS) and complete source are on the [release page](https://github.com/mbhall88/pterobacter/releases/tag/0.1.0).

### Cargo

With Rust and GNU C++ with C++17 support installed:

```sh
cargo install pterobacter --version 0.1.0 --locked
ptero --version
```

On macOS, install the Xcode command-line tools and Homebrew GCC first. See [source-build prerequisites](https://mbhall88.github.io/pterobacter/installation.html#macos-source-build). Cargo's bin directory, usually `~/.cargo/bin`, must be on your `PATH`. Both `ptero` and `pterobacter` run the same program.

### Other installation methods

A Linux amd64 image is available at `ghcr.io/mbhall88/pterobacter:0.1.0`; see [container usage](https://mbhall88.github.io/pterobacter/containers.html). A Bioconda recipe is being prepared; the package is not yet available from that channel.

## Classify an assembly

Install the database, then classify an assembly:

```sh
ptero database install 2026-10-15
ptero --output isolate.json --report-dir isolate-report isolate.fa
```

The database includes reference genomes and an MLST scheme. This build uses version `2026-10-15`, published on [Figshare](https://doi.org/10.6084/m9.figshare.34640883.v1). See [database management](https://mbhall88.github.io/pterobacter/databases.html) for storage locations and verification.

Pterobacter estimates average nucleotide identity (ANI) with skani and uses genome alignments (ANIm) to investigate eligible samples near the species boundary. MLST runs by default. Use `--no-anim` to skip the alignment step or `--no-mlst` to skip sequence typing. You can also pass `--no-mlst` to `database install` to omit the scheme download.

Open `isolate-report/report.md` for the readable result or `isolate-report/calls.tsv` for a table of calls. The JSON report and `evidence.tsv` contain the detailed comparisons. Output files and directories must be new.

Try the [public-genome examples](https://mbhall88.github.io/pterobacter/examples/index.html): download two assemblies and see a species/subspecies call, ST286 and a genomovar match, alongside an unresolved case with ST2274. [Reading results](https://mbhall88.github.io/pterobacter/results.html) explains the output fields. For an uncertain assignment, the [worked boundary-review example](https://mbhall88.github.io/pterobacter/boundary-review.html#a-recorded-wider-context-example) shows how to examine genome comparisons and tree evidence.

## Run a batch

```sh
ptero --jobs 2 --threads 2 --output batch.json --report-dir batch-report \
  isolate_1.fa isolate_2.fa
```

This processes two assemblies at a time, with two search threads per assembly. You can also provide a [file list or sample sheet](https://mbhall88.github.io/pterobacter/running.html#a-list-of-assemblies-or-named-samples).

## Documentation

The [user guide](https://mbhall88.github.io/pterobacter/) covers:

- [Running assemblies](https://mbhall88.github.io/pterobacter/running.html) and [command-line options](https://mbhall88.github.io/pterobacter/cli-help.html)
- [Supported taxa](https://mbhall88.github.io/pterobacter/supported-taxa.html) and naming conventions
- [MLST](https://mbhall88.github.io/pterobacter/historical-mlst.html), including scheme coverage and using your own scheme
- [Genomovar matches](https://mbhall88.github.io/pterobacter/genomovar.html)
- [Investigating uncertain results](https://mbhall88.github.io/pterobacter/boundary-review.html) and [troubleshooting](https://mbhall88.github.io/pterobacter/troubleshooting.html)

## Cite and contribute

Use [CITATION.cff](CITATION.cff) when citing Pterobacter, and cite the [reference database](https://doi.org/10.6084/m9.figshare.34640883.v1) separately.

<!-- Maintainer TODO: add the accompanying paper citation and DOI here and in CITATION.cff when available. -->

[Contributing](CONTRIBUTING.md) covers development and checks. The combined executable is distributed under [GPL-3.0-or-later](LICENSE-GPL-3.0.txt); Pterobacter-authored source retains its [MIT licence](LICENSE). See [licensing and source availability](https://mbhall88.github.io/pterobacter/licensing.html) for component terms.
