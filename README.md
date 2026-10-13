# Terobacter

Classify *Enterobacter* isolate assemblies and report supported species and subspecies calls, the closest references, and reasons for uncertainty. Give Terobacter a whole assembly in FASTA format, including its plasmids and draft contigs. Reads and metagenomes are not supported.

Private development preview: distribution is on hold while the repository is reviewed. The current source reports version `0.0.0`.

## Install

Collaborators with access to the private repository can build from source. You need Rust and GNU C++ with C++17 support.

```sh
git clone git@github.com:mbhall88/terobacter.git
cd terobacter
cargo install --path . --locked
tero --version
```

On macOS, install Xcode command-line tools and Homebrew GCC, and set `CXX` to its GNU `g++` executable before building. See [installation](docs/installation.md) for compiler setup, offline builds and platform limits. Public binaries and package-manager distributions are not available yet.

## Classify an assembly

Obtain the reference directory and its trusted manifest SHA-256 through the private handoff. In this example the directory is named `reference`, and `REFERENCE_SHA256` contains that supplied digest. [Reference setup](docs/reference-setup.md) explains verification and the optional data components.

```sh
tero --reference-package reference --expected-sha256 "$REFERENCE_SHA256"   --output isolate.json --report-dir isolate-report isolate.fa
```

`tero` and `terobacter` are equivalent commands. Open `isolate-report/report.md` for the readable result, or `isolate-report/calls.tsv` for one row per assembly. `isolate.json` and `isolate-report/report.json` retain the detailed report; `evidence.tsv` contains the comparisons. Output files and report directories must be new.

Here are selected fields from a synthetic report fixture, not a biological result:

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

The species and subspecies are assigned in this example, but no genomovar is assigned. An unresolved call keeps its name fields empty; the closest reference is evidence, not a replacement label. Check reasons and warnings in the readable report. [Reading results](docs/results.md) explains these cases and the fixture source.

## Run a batch

```sh
tero --reference-package reference --expected-sha256 "$REFERENCE_SHA256"   --jobs 2 --threads 2 --output batch.json --report-dir batch-report   isolate_1.fa isolate_2.fa
```

Each input has its own result and execution status. Two jobs with two threads each use up to four search threads. Once the program and data are provisioned, analysis needs no network or external analysis program.

## Guides

The [user guide](docs/usage.md) covers reference setup, running assemblies, results and troubleshooting. See [supported taxa](docs/supported-taxa.md) for naming and sampling limits, [historical MLST](docs/historical-mlst.md) for optional typing, and [the boundary-review example](docs/boundary-review.md) for an uncertain result needing wider context.

A genomovar result is a conditional match to a reference definition. An unrepresented group can match a neighbouring group; overlapping matches are withheld, and one-member groups carry a sparse-reference warning. See [genomovar matches](docs/genomovar-guide.md). Quality control remains your responsibility.

## Cite and contribute

Use the software metadata in [CITATION.cff](CITATION.cff) when citing Terobacter. [Contributing](CONTRIBUTING.md) covers repository checks and development commands.

Application code is [MIT licensed](LICENSE); bundled components have their own [licences and notices](THIRD-PARTY-LICENSES.md).
