# Try two real genomes

These examples use public NCBI assemblies and the published database **2026-10-15**. They show what the tool actually writes, including MLST alleles, a genomovar match and an unresolved species call.

| Assembly | Strain | Pterobacter result | MLST | Genomovar |
|---|---|---|---|---|
| [GCA_013738375.1](https://www.ncbi.nlm.nih.gov/datasets/genome/GCA_013738375.1/) | RHBSTW-00333 | *E. xiangfangensis* subsp. *hoffmannii* | ST286 | GV10 |
| [GCA_043966805.1](https://www.ncbi.nlm.nih.gov/datasets/genome/GCA_043966805.1/) | ECC-219 | Species unresolved after ANIm | ST2274 | Withheld |

[Walk through the assigned genome](public/assigned.md) · [Walk through the unresolved genome](public/unresolved.md)

NCBI calls the first assembly *E. hormaechei* and the second *Enterobacter* sp. ECC-219. Pterobacter uses its [documented taxonomic framework](../supported-taxa.md); the *xiangfangensis* subsp. *hoffmannii* combination is proposed. These are demonstrations, not an independent accuracy test. The first genome is itself a diversity reference and a member of the database's GV10 definition.

## Run them yourself

After [installing Pterobacter](../installation.md), download and run the [reproduction script](public/reproduce.sh). It requires Bash, curl, gzip and either `sha256sum` or `shasum`:

```sh
curl --fail --location \
  https://mbhall88.github.io/pterobacter/examples/public/reproduce.sh \
  --output reproduce.sh
bash reproduce.sh pterobacter-example
```

The script creates a new directory and refuses to overwrite an existing one. It downloads both whole assemblies, verifies their FASTA checksums, installs database version 2026-10-15 including MLST, and runs one assembly and a two-assembly batch. Allow roughly 1 GB for the database, inputs and results, plus alignment working space. Use local storage that supports the database installer's atomic operations.

The classification commands inside that directory are:

```sh
ptero --database-dir ./database --threads 2 --sample-name RHBSTW-00333 \
  --anim-output single-alignments --output single.json --report-dir single \
  GCA_013738375.1.fna

ptero --database-dir ./database --threads 2 --jobs 1 --sample-sheet samples.tsv \
  --anim-output batch-alignments --output batch.json --report-dir batch
```

Both assemblies are supplied whole. RHBSTW-00333 has eight records, including its plasmids; ECC-219 has one chromosome record. MLST and ANIm are enabled by default. Only ECC-219 needs ANIm in these examples.

## Inspect the saved outputs

| Run | Readable report | Calls | Comparisons | Full JSON |
|---|---|---|---|---|
| RHBSTW-00333 alone | [Markdown](public/single/report.md) | [TSV](public/single/calls.tsv) | [TSV](public/single/evidence.tsv) | [JSON](public/single/report.json) |
| Both assemblies | [Markdown](public/batch/report.md) | [TSV](public/batch/calls.tsv) | [TSV](public/batch/evidence.tsv) | [JSON](public/batch/report.json) |

The files are unedited outputs from source commit `0944f6d13b989f0329a6c266dbc9e20340fde58d`, which reports software version `0.0.0` before the first release. [Provenance](public/provenance.json) records the source, database, input and output hashes. The [checksums](public/SHA256SUMS) cover the saved reports and reproduction script.

Your local paths, process identifiers, timings, resource limits and software version will differ. Use the recorded assembly and database hashes when comparing results; byte-identical report files are not expected across machines. The saved reports retain the tool's `not_yet_validated` metadata value; these examples do not change that status.

MLST uses PubMLST submissions through **31 December 2024**. An ST describes an allele profile, not a species assignment: ECC-219 demonstrates that distinction. See [MLST](../historical-mlst.md) for scheme limits and [genomovars](../genomovar.md) for the scope of a GV match.
