# Analyse assemblies

Pass one whole isolate assembly per FASTA input. Keep plasmids, draft contigs and short records: no manual chromosome selection is required. Gzip, bzip2, xz and zstd FASTA are supported. Reads, metagenomes and mixed samples are outside the supported input.

First [install the program](installation.md) and [set up the reference variables](reference-setup.md).

## One assembly

```sh
tero --reference-package "$REFERENCE_PACKAGE" --expected-sha256 "$REFERENCE_SHA256" \
  --anim-reference-package "$ANIM_REFERENCE_PACKAGE" --expected-anim-sha256 "$ANIM_SHA256" \
  --anim-output isolate-alignments \
  --threads 2 --output isolate.json --report-dir isolate-report isolate.fa
```

Open `isolate-report/report.md` for the answer and warnings. The full JSON is in `isolate.json` and `isolate-report/report.json`; [reading results](results.md) explains the fields. Without `--output`, the full JSON goes to standard output. `--report-dir` adds the report files without changing that destination.

## A batch

```sh
tero --reference-package "$REFERENCE_PACKAGE" --expected-sha256 "$REFERENCE_SHA256" \
  --anim-reference-package "$ANIM_REFERENCE_PACKAGE" --expected-anim-sha256 "$ANIM_SHA256" \
  --anim-output batch-alignments \
  --jobs 2 --threads 2 --output batch.json --report-dir batch-report \
  isolate_1.fa isolate_2.fa
```

The main JSON is an array in input order. `calls.tsv` has one row per assembly, and `report.md` has a section for each. Inspect every input's execution status; a failure in one input does not erase a completed result from another. `--jobs` controls concurrent assemblies; `--threads` controls the embedded search threads within each assembly. Two jobs with two threads each allow up to four search threads. See the [complete synthetic batch outputs](examples/README.md#batch-outputs).

All output files and report directories must be new, distinct, outside reference-package directories and not nested within one another. Outputs can sit beside the input assembly, but must not replace it. The program refuses existing paths. Analysis makes no network request after provisioning.

## Common controls

| Option | Use |
|---|---|
| `-t`, `--threads N` | Search threads per assembly; default 1. |
| `-j`, `--jobs N` | Assemblies analysed concurrently; default 1. |
| `-o`, `--output FILE` | Save the JSON to a new file. |
| `-d`, `--report-dir DIR` | Add readable Markdown and TSV reports in a new directory. |
| `-n`, `--no-anim` | Use skani only, without alignment rescue. Omit the alignment package/evidence options with this flag. |
| `-h`, `--help` | Show the complete current option list. |

[Historical MLST](historical-mlst.md) is optional. The [command reference](assembly.md) covers boundary ANIm, its reference components, resource limits and verified alignment caching. The default uses skani plus MUMmer4 rescue. For skani only, use:

```sh
tero -n -r "$REFERENCE_PACKAGE" -R "$REFERENCE_SHA256" -t 2 -o isolate.json -d isolate-report isolate.fa
```

[Command-line help](cli-help.md) lists every short and long option.
