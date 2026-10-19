# Analyse assemblies

Pass one whole isolate assembly per FASTA input. Keep plasmids, draft contigs and short records: no manual chromosome selection is required. Gzip, bzip2, xz and zstd FASTA are supported. Reads, metagenomes and mixed samples are outside the supported input.

First [install the program](installation.md) and [install the database](databases.md).

## One assembly

```sh
ptero --threads 2 --output isolate.json --report-dir isolate-report isolate.fa
```

Open `isolate-report/report.md` for the answer and warnings. The full JSON is in `isolate.json` and `isolate-report/report.json`; [reading results](results.md) explains the fields. Without `--output`, the full JSON goes to standard output. `--report-dir` adds the report files without changing that destination.

## A batch

```sh
ptero --jobs 2 --threads 2 --output batch.json --report-dir batch-report \
  isolate_1.fa isolate_2.fa
```

The main JSON is an array in input order. `calls.tsv` has one row per assembly, and `report.md` has a section for each. Inspect every input's execution status; a failure in one input does not erase a completed result from another. `--jobs` controls concurrent assemblies; `--threads` controls the embedded search threads within each assembly. Two jobs with two threads each allow up to four search threads. See the [complete synthetic batch outputs](examples/README.md#batch-outputs).

## A list of assemblies or named samples

For many files, put the paths in a list instead of on the command line:

```sh
ptero -j 2 -t 2 -o batch.json -d batch-report --input-list assemblies.txt
```

`assemblies.txt` holds one assembly path per line, as UTF-8 text. Blank lines are skipped and Windows line endings are accepted. Every other character on a line belongs to the path, including spaces, and there is no comment syntax. A relative path is read from the directory that holds the list, not from where you run the command.

To report your own sample names, use a sample sheet instead. It is a tab-separated UTF-8 file with the header `sample` and `path`, then one row per assembly:

```text
sample	path
EC-001	assemblies/EC-001 contigs.fa.gz
EC-002	/data/run7/EC-002.fasta
```

```sh
ptero -o batch.json -d batch-report --sample-sheet samples.tsv
```

Each row needs exactly two fields, both filled. Names and paths must each appear once; two spellings of the same file, or a link to it, count as the same path. For a single assembly given on the command line, `--sample-name NAME` sets its name.

Use one input source per run: assemblies on the command line, `--input-list` or `--sample-sheet`. `--sample-name` works only with one command-line assembly. Reports keep the supplied order. Each input still gets its own execution status, so a missing or unreadable file fails only its own row. A malformed list or sheet, a repeated name or path, or a conflicting option stops the run with exit code 2 before any output is created.

A sample name is a label for the reports. It appears as `input.sample_name` in the JSON, in the `sample_name` column of both TSV files and in the section headings of `report.md`. It never names an output file: the files inside `--report-dir` keep their fixed names. The analysis uses the file path, exactly as it would for the same path given on the command line, so naming a sample never changes a measurement or a call.

All output files and report directories must be new, distinct, outside reference-package directories and not nested within one another. Outputs can sit beside the input assembly, but must not replace it. The program refuses existing paths. Analysis makes no network request after provisioning.

## Common controls

| Option | Use |
|---|---|
| `-t`, `--threads N` | Search threads per assembly; default 1. |
| `-j`, `--jobs N` | Assemblies analysed concurrently; default 1. |
| `-f`, `--input-list FILE` | Read assembly paths from a file, one per line. |
| `-F`, `--sample-sheet FILE` | Read sample names and assembly paths from a TSV with the header `sample` and `path`. |
| `-N`, `--sample-name NAME` | Name the one assembly given on the command line. |
| `-o`, `--output FILE` | Save the JSON to a new file. |
| `-d`, `--report-dir DIR` | Add readable Markdown and TSV reports in a new directory. |
| `-n`, `--no-anim` | Use skani only, without alignment rescue. Omit the alignment package/evidence options with this flag. |
| `-h`, `--help` | Show the complete current option list. |

[Historical MLST](historical-mlst.md) is optional. The [command reference](assembly.md) covers boundary ANIm, its reference components, resource limits and verified alignment caching. The default uses skani plus MUMmer4 rescue. For skani only, use:

```sh
ptero -n -r "$REFERENCE_PACKAGE" -R "$REFERENCE_SHA256" -t 2 -o isolate.json -d isolate-report isolate.fa
```

[Command-line help](cli-help.md) lists every short and long option.
