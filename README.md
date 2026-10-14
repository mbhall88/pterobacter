# Pterobacter

<img src="docs/assets/pterobacter.png" alt="Pterobacter mascot" width="180" align="right">

Pronounced with a silent P, like pterodactyl.

[![CI](https://github.com/mbhall88/terobacter/actions/workflows/ci.yml/badge.svg)](https://github.com/mbhall88/terobacter/actions/workflows/ci.yml) [![User guide](https://img.shields.io/badge/docs-user_guide-236b73)](docs/usage.md)

Classify *Enterobacter* isolate assemblies and report supported species and subspecies calls, the closest references, and reasons for uncertainty. Give Pterobacter a whole assembly in FASTA format, including its plasmids and draft contigs.

## Install

### From source

You need Rust and GNU C++ with C++17 support. On macOS, first install the Xcode command-line tools and Homebrew GCC:

```sh
xcode-select --install  # skip if already installed
brew install gcc
export CXX="$(find "$(brew --prefix gcc)/bin" -name 'g++-[0-9]*' | head -n 1)"
```

```sh
git clone git@github.com:mbhall88/terobacter.git
cd terobacter
cargo install --path . --locked
ptero --version
```

See [installation](docs/installation.md) for compiler setup, offline builds and platform limits. Automatic Homebrew GCC discovery is awaiting review in [PR #38](https://github.com/mbhall88/terobacter/pull/38).

### Cargo

Registry installation is planned. For now, use `cargo install --path . --locked` from the source checkout above.

### Conda

A Bioconda package is planned; it is not available yet.

### Containers

Container images are planned; they are not available yet.

### Precompiled binaries

Release downloads are planned; they are not available yet.

## Classify an assembly

Obtain the reference directory and its trusted manifest SHA-256 through the private handoff. In this example the directory is named `reference`, and `REFERENCE_SHA256` contains that supplied digest. [Reference setup](docs/reference-setup.md) explains verification and the optional data components.

```sh
ptero --reference-package reference --expected-sha256 "$REFERENCE_SHA256"   --output isolate.json --report-dir isolate-report isolate.fa
```

`ptero` and `pterobacter` are equivalent commands. Open `isolate-report/report.md` for the readable result, or `isolate-report/calls.tsv` for one row per assembly. `isolate.json` and `isolate-report/report.json` retain the detailed report; `evidence.tsv` contains the comparisons. Choose their names with `--output FILE` and `--report-dir DIR`; both paths must be new. The four filenames inside the report directory are fixed.

Here are selected fields from an example report fixture:

```json
{
  "assignment": {
    "status": "assigned",
    "species_name": "Enterobacter syntheticus",
    "subspecies_name": "Enterobacter syntheticus subsp. syntheticus",
    "genomovar_id": null
  },
  "genomovar": {
    "status": "withheld",
    "genomovar_id": null,
    "reasons": ["absent_eligible_parent"]
  }
}
```

This synthetic fixture assigns species and subspecies while withholding the genomovar. An assigned genomovar looks like this in the synthetic policy fixture:

```json
{
  "genomovar": {
    "genomovar_id": "GV01",
    "reasons": [],
    "status": "assigned"
  }
}
```

`GV01` is the fixture's short ID. The reference package uses versioned IDs such as `ECTX-GV-2026-10-02-v1-GV01`. [Example files](docs/examples/README.md) include complete JSON, TSV and Markdown outputs and explain their sources.

For an observed public-assembly example, the reviewed case `VAL-L3-1B0C8D325620` matched the proposed *E. xiangfangensis* subsp. *hoffmannii* group. Its closest diversity reference had 99.97% ANI, 93.65% query aligned fraction and 92.61% reference aligned fraction. The [worked example](docs/boundary-review.md#a-recorded-wider-context-example) includes its saved comparisons and tree review. An unresolved call keeps its name fields empty; the closest reference is evidence, not a replacement label. Check reasons and warnings in the readable report. [Reading results](docs/results.md) explains these cases and the fixture source.

## Run a batch

```sh
ptero --reference-package reference --expected-sha256 "$REFERENCE_SHA256"   --jobs 2 --threads 2 --output batch.json --report-dir batch-report   isolate_1.fa isolate_2.fa
```

`--jobs 2` processes two assemblies concurrently. `--threads 2` gives each assembly two threads for the embedded search, for up to four search threads in total. Each input has its own result and execution status. See the [batch JSON](docs/examples/output/batch/report.json), [calls table](docs/examples/output/batch/calls.tsv) and [readable report](docs/examples/output/batch/report.md) for a synthetic batch with both completed and failed inputs.

## Guides

The [user guide](docs/usage.md) covers reference setup, running assemblies, results and troubleshooting. See [supported taxa](docs/supported-taxa.md) for naming and sampling limits, [historical MLST](docs/historical-mlst.md) for optional typing, and [the boundary-review example](docs/boundary-review.md) for an uncertain result needing wider context.

A genomovar result is a conditional match to a reference definition. An unrepresented group can match a neighbouring group; overlapping matches are withheld, and one-member groups carry a sparse-reference warning. See [genomovar matches](docs/genomovar.md). Quality control remains your responsibility.

Upgrading from a development checkout? See the [naming migration](docs/migration.md).

## Cite and contribute

Use the software metadata in [CITATION.cff](CITATION.cff) when citing Pterobacter.

<!-- Maintainer TODO: add the accompanying paper citation and DOI here and in CITATION.cff when available. --> [Contributing](CONTRIBUTING.md) covers repository checks and development commands.

Application code is [MIT licensed](LICENSE); bundled components have their own [licences and notices](THIRD-PARTY-LICENSES.md).
