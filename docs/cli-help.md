# Command-line help

For a first run, follow [classify an assembly](running.md). This page explains the less familiar options, followed by the exact help printed by the current executable.

## ANIm and ANI thresholds

Pterobacter first estimates ANI with skani. For eligible samples just below the species boundary, it also runs ANIm using whole-genome alignments to refine the estimate. ANIm can take longer, especially in large batches with many borderline samples; samples outside that band do not incur alignment work. Use `-n` or `--no-anim` to skip ANIm and use skani only.

`-s` / `--species-ani PERCENT` sets the minimum species ANI, **96% by default**. Enter a percentage such as `96.5`, not a fraction such as `0.965`. A skani species call requires ANI greater than or equal to that value; an ANIm-supported call requires ANI strictly greater than it in both alignment directions. Coverage and competitor checks must also pass.

`-S` / `--subspecies-ani PERCENT` sets the minimum skani ANI for a subspecies call, **98% by default**. ANI must be greater than or equal to this value, with the other subspecies checks satisfied. ANIm never assigns subspecies.

Require `80 <= species threshold <= subspecies threshold <= 100`. A custom species threshold also moves the ANIm trigger band to the interval from 1.5 percentage points below that threshold, inclusive, up to the threshold, exclusive, with a lower bound of 80%. Supplying either threshold marks the result `custom_unvalidated`, even when you supply its default number. Leave both out to use the standard settings. Custom thresholds cannot be combined with `--no-anim`.

The input is always an isolate assembly FASTA. Every public option has a short form, shown below.

## MLST

MLST is off by default in this build. To request it, provide the scheme directory with `--mlst-package` and its trusted manifest hash with `--mlst-expected-sha256`. [The MLST guide](historical-mlst.md) has the working command and explains the scheme's date limit. Selecting an installed scheme without these paths, and accepting newer scheme versions, are planned in [issue39](https://github.com/mbhall88/terobacter/issues/39).

## Alignment files and resource limits

The `--anim-` options apply to the MUMmer4 rescue mode. ANIm means average nucleotide identity measured from genome alignments.

| Option | What it is for |
|---|---|
| `--anim-reference-package` | The matching reference genome FASTAs, needed when the rescue runs alignments. |
| `--expected-anim-sha256` | The trusted hash of that FASTA package's `anim.json` manifest. |
| `--anim-output` | A new directory for alignments and execution records, including input snapshots, errors and hashes. It preserves evidence used in the call. |
| `--anim-cache` | A reusable store of verified query/reference comparisons to avoid repeating matching work. |
| `--anim-pair-seconds` | Maximum elapsed time for one query/reference pair, including both alignment directions. Default: 120 seconds. |
| `--anim-query-seconds` | Total alignment time budget for one input assembly. Default: 600 seconds. |
| `--anim-memory-mib` | Memory ceiling for each native alignment worker. Default: 1024 MiB. |

`--report-dir` holds the classification reports; `--anim-output` holds the underlying alignment evidence. Use separate new directories. The [rescue command](assembly.md#boundary-anim) shows both the required inputs and evidence output; [cache reuse](assembly.md#reusing-reciprocal-pairs) describes the optional cache.

The current limits bound native alignment work. Reaching a limit leaves the required comparison incomplete and withholds the call; it is not evidence that the assembly belongs to a different species. You can raise the limits up to 1800 seconds per pair, 7200 seconds per query and 6144 MiB per worker, but this build has no unlimited setting. Ctrl-C requests a stop and retains partial evidence. [Issue43](https://github.com/mbhall88/terobacter/issues/43) tracks the request to make these limits optional; the current values are listed here so the guide matches the executable.

## Exact terminal output

Captured from the current `ptero --help`. The `pterobacter` alias accepts the same options.

```text
Standalone, self-contained Enterobacter isolate classification

Usage: ptero [OPTIONS] --reference-package <REFERENCE_PACKAGE> --expected-sha256 <EXPECTED_SHA256> <ASSEMBLY>...

Arguments:
  <ASSEMBLY>...  Isolate assembly FASTA files, analysed whole and as supplied

Options:
  -s, --species-ani <PERCENT>
          Minimum species ANI (%): skani >= this value; ANIm must be strictly greater [default: 96]
  -S, --subspecies-ani <PERCENT>
          Minimum subspecies ANI (%): skani >= this value [default: 98]
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
          Historical MLST scheme package directory; requests optional supplementary MLST
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
          Reciprocal pair wall timeout, default 120 seconds, maximum 1800
  -Q, --anim-query-seconds <ANIM_QUERY_SECONDS>
          Total ANIm wall budget per query, default 600 seconds, maximum 7200
  -L, --anim-memory-mib <ANIM_MEMORY_MIB>
          Native worker memory ceiling, default 1024 MiB, maximum 6144 MiB
  -h, --help
          Print help
  -V, --version
          Print version

Uses skani first, then ANIm for eligible samples near the species boundary.
ANIm can be slower; use -n/--no-anim to skip it.
ANI thresholds are percentages, e.g. --species-ani 96.5. Require 80 <= species <= subspecies <= 100.
Custom thresholds cannot be combined with --no-anim and mark results custom_unvalidated.
Guide: https://github.com/mbhall88/terobacter/blob/main/docs/assembly.md
```
