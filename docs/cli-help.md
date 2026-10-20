# Command-line help

For a first run, follow [classify an assembly](running.md). This page explains the less familiar options, followed by the exact help printed by the current executable.

## ANIm and ANI thresholds

Pterobacter first estimates ANI with skani. For eligible samples just below the species boundary, it also runs ANIm using whole-genome alignments to refine the estimate. ANIm can take longer, especially in large batches with many borderline samples; samples outside that band do not incur alignment work. Use `-n` or `--no-anim` to skip ANIm and use skani only.

`-s` / `--species-ani PERCENT` sets the minimum species ANI, **96% by default**. Enter a percentage such as `96.5`, not a fraction such as `0.965`. A skani species call requires ANI greater than or equal to that value; an ANIm-supported call requires ANI strictly greater than it in both alignment directions. Coverage and competitor checks must also pass.

`-S` / `--subspecies-ani PERCENT` sets the minimum skani ANI for a subspecies call, **98% by default**. ANI must be greater than or equal to this value, with the other subspecies checks satisfied. ANIm never assigns subspecies.

Require `80 <= species threshold <= subspecies threshold <= 100`. A custom species threshold also moves the ANIm trigger band to the interval from 1.5 percentage points below that threshold, inclusive, up to the threshold, exclusive, with a lower bound of 80%. Supplying either threshold marks the result `custom_unvalidated`, even when you supply its default number. Leave both out to use the standard settings. Custom thresholds cannot be combined with `--no-anim`.

The input is always an isolate assembly FASTA. Every public option has a short form, shown below.

## Assembly lists and sample names

Assemblies can come from the command line, from `-f` / `--input-list FILE` with one path per line, or from `-F` / `--sample-sheet FILE`, a TSV with the header `sample` and `path`. Use one source per run. `-N` / `--sample-name NAME` names a single command-line assembly. Relative paths in a list or sheet are read from the file's own directory. Names label the reports only; the analysis uses the file path as it would for a command-line argument. [Running Pterobacter](running.md#a-list-of-assemblies-or-named-samples) has the exact file rules and examples.

## MLST

MLST runs by default. Use `-x/--no-mlst` to skip it. For explicit packages, provide `--mlst-package` and `--mlst-expected-sha256` explicitly. [Database management](databases.md) includes a working local install example; [MLST](historical-mlst.md) explains the submission cutoff, and the [current/custom contract](contracts/MLST-PACKAGE-v2.md) defines compatible newer local packages. The bundled scheme is downloaded with the managed database; automatic provisioning of newer PubMLST records is not available.

## Alignment files and resource limits

The `--anim-` options apply to the MUMmer4 rescue mode. ANIm means average nucleotide identity measured from genome alignments.

| Option | What it is for |
|---|---|
| `--anim-reference-package` | The matching reference genome FASTAs, needed when the rescue runs alignments. |
| `--expected-anim-sha256` | The trusted hash of that FASTA package's `anim.json` manifest. |
| `--anim-output` | A new directory for alignments and execution records, including input snapshots, errors and hashes. It preserves evidence used in the call. |
| `--anim-cache` | A reusable store of verified query/reference comparisons to avoid repeating matching work. |
| `--anim-pair-seconds` | Opt-in wall limit for one reciprocal pair, including both directions; omitted or zero means unlimited. |
| `--anim-query-seconds` | Opt-in ANIm-stage budget for one assembly; omitted or zero means unlimited. |
| `--anim-memory-mib` | Opt-in worker ceiling in MiB; omitted or zero means unlimited. |

`--report-dir` holds the classification reports; `--anim-output` holds the underlying alignment evidence. Use separate new directories. The [rescue command](assembly.md#boundary-anim) shows both the required inputs and evidence output; [cache reuse](assembly.md#reusing-reciprocal-pairs) describes the optional cache.

Execution has no tool time or memory ceiling unless you supply a positive limit. Each control is independent; omission or zero disables it. The query control covers the ANIm stage for one assembly, with cooperative checks after synchronous operations. It does not time skani, genomovar, MLST or the whole batch. OS/scheduler restrictions and jobs/thread bounds still apply. A limit failure leaves required evidence incomplete and withholds the call. Ctrl-C requests a stop and retains partial evidence when possible; repeating the signal terminates immediately. [Execution details](assembly.md#execution-and-historical-evidence) describe Linux/macOS enforcement, inherited limits and historical cache compatibility.

## Exact terminal output

Captured from the current `ptero --help`. The `pterobacter` alias accepts the same options.

```text
Standalone, self-contained Enterobacter isolate classification

Usage: ptero [OPTIONS] [ASSEMBLY]...
       ptero [OPTIONS] [ASSEMBLY]... <COMMAND>

Commands:
  database  Install, list or verify pinned reference and typing components
  help      Print this message or the help of the given subcommand(s)

Arguments:
  [ASSEMBLY]...  Isolate assembly FASTA files, analysed whole and as supplied

Options:
  -D, --database <DATABASE>
          Exact installed database version; otherwise use the catalogue's published default
  -B, --database-dir <DIR>
          Managed database store on a filesystem supporting atomic no-replace rename
  -C, --catalogue <CATALOGUE>
          Explicit trusted catalogue file; requires its independently supplied SHA-256
  -H, --catalogue-sha256 <CATALOGUE_SHA256>
          Expected SHA-256 of the explicit catalogue
  -x, --no-mlst
          Skip MLST sequence typing
  -s, --species-ani <PERCENT>
          Minimum species ANI (%): skani >= this value; ANIm must be strictly greater [default: 96]
  -S, --subspecies-ani <PERCENT>
          Minimum subspecies ANI (%): skani >= this value [default: 98]
  -f, --input-list <FILE>
          Read assembly paths from this file, one per line; relative paths are from its directory
  -F, --sample-sheet <FILE>
          Read sample names and assembly paths from this TSV with the header sample<TAB>path
  -N, --sample-name <NAME>
          Name to report for the one assembly given; reporting only, never sent to skani
  -r, --reference-package <REFERENCE_PACKAGE>
          Authenticated reference package directory
  -R, --expected-sha256 <EXPECTED_SHA256>
          Expected SHA-256 of portable.json, obtained from a trusted source
  -n, --no-anim
          Skip ANIm near the species boundary; use skani only
  -t, --threads <THREADS>
          Worker threads for each assembly [default: 1]
  -j, --jobs <JOBS>
          Assemblies analysed at the same time, each with its own --threads [default: 1]
  -o, --output <FILE>
          Write the JSON report to this new file instead of standard output; never replaced
  -m, --mlst-package <MLST_PACKAGE>
          MLST scheme package directory; overrides the managed scheme
  -M, --mlst-expected-sha256 <MLST_EXPECTED_SHA256>
          Expected SHA-256 of the MLST package manifest.json, obtained from a trusted source
  -d, --report-dir <DIR>
          Write report.json, calls.tsv, evidence.tsv and report.md into this new directory
  -a, --anim-reference-package <ANIM_REFERENCE_PACKAGE>
          Reference genome FASTAs for MUMmer4 rescue; needed when alignments are triggered
  -A, --expected-anim-sha256 <EXPECTED_ANIM_SHA256>
          Trusted SHA-256 of the component's anim.json
  -e, --anim-output <ANIM_OUTPUT>
          New directory for alignments and execution records; see the assembly guide
  -c, --anim-cache <ANIM_CACHE>
          Directory of verified reciprocal pairs to reuse and extend; created when absent
  -T, --anim-pair-seconds <ANIM_PAIR_SECONDS>
          Opt-in reciprocal pair wall limit in seconds; omitted or 0 means unlimited
  -Q, --anim-query-seconds <ANIM_QUERY_SECONDS>
          Opt-in ANIm-stage budget per assembly in seconds; omitted or 0 means unlimited
  -L, --anim-memory-mib <ANIM_MEMORY_MIB>
          Opt-in native worker ceiling in MiB; omitted or 0 means unlimited
  -h, --help
          Print help
  -V, --version
          Print version

Uses skani first, then ANIm for eligible samples near the species boundary.
ANIm can be slower; use -n/--no-anim to skip it.
MLST runs by default; use -x/--no-mlst to skip sequence typing.
Give assemblies as arguments, or list them with -f/--input-list or -F/--sample-sheet.
Sample names label reports only; analysis uses each file path.
ANI thresholds are percentages, e.g. --species-ani 96.5. Require 80 <= species <= subspecies <= 100.
Custom thresholds cannot be combined with --no-anim and mark results custom_unvalidated.
Guide: https://mbhall88.github.io/pterobacter/assembly.html
```
