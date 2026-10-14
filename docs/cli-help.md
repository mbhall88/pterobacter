# Command-line help

For a first run, follow [classify an assembly](running.md). This page explains the less familiar options, followed by the exact help printed by the current executable.

## Classification mode and ANI thresholds

The default uses **skani plus MUMmer4 rescue**, adding whole-genome alignments for eligible uncertain species calls. Use `-n` or `--no-anim` for skani-only classification. The internal names `conservative-assembly-v1` and `conservative-assembly-anim-v2` identify these calling rules; they are not software release numbers. See [the comparison and selection command](assembly.md#classification-modes).

`--species-ani` and `--subspecies-ani` change the minimum average nucleotide identity (ANI) percentages for species and subspecies calls. They are advanced options for the default rescue mode, with defaults of 96 and 98 respectively. Supplying either option marks the result `custom_unvalidated`, even if you supply the default number. Leave them out to use the standard rules. [Threshold details](v2-decisions.md#custom-ani-thresholds) explain which comparisons and safeguards apply.

Assemblies are already the only supported input. Omit `--input-kind`; it defaults to `assembly` and does not enable read classification. The explicit input-kind option remains available for compatibility.

`--policy` (`-p`) and `--assignment-policy` accept the existing policy IDs. Combining either with `--no-anim` is an error. Custom ANI thresholds and alignment options are also incompatible with skani-only mode. Every public option has a short form, shown in the terminal output below.

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

`--report-dir` holds the classification reports; `--anim-output` holds the underlying alignment evidence. Use separate new directories. The [rescue command](assembly.md#candidate-v2-anim-rescue) shows both the required inputs and evidence output; [cache reuse](assembly.md#reusing-reciprocal-pairs) describes the optional cache.

The current limits bound native alignment work. Reaching a limit leaves the required comparison incomplete and withholds the call; it is not evidence that the assembly belongs to a different species. You can raise the limits up to 1800 seconds per pair, 7200 seconds per query and 6144 MiB per worker, but this build has no unlimited setting. Ctrl-C requests a stop and retains partial evidence. [Issue43](https://github.com/mbhall88/terobacter/issues/43) tracks the request to make these limits optional; the current values are listed here so the guide matches the executable.

## Exact terminal output

Captured from the current `tero --help`. The `terobacter` alias accepts the same options.

```text
Standalone, self-contained Enterobacter isolate classification

Usage: tero [OPTIONS] --reference-package <REFERENCE_PACKAGE> --expected-sha256 <EXPECTED_SHA256> <ASSEMBLY>...

Arguments:
  <ASSEMBLY>...  Isolate assembly FASTA files, analysed whole and as supplied

Options:
  -s, --species-ani <T>
          Custom species ANI percentage for rescue mode; marks results custom_unvalidated
  -S, --subspecies-ani <S>
          Custom subspecies ANI percentage for rescue mode; requires 80 <= T <= S <= 100
  -r, --reference-package <REFERENCE_PACKAGE>
          Authenticated reference package directory
  -R, --expected-sha256 <EXPECTED_SHA256>
          Expected SHA-256 of portable.json, obtained from a trusted source
  -p, --policy <POLICY>
          Explicit calling rules (compatibility option); use --no-anim for skani only [default: conservative-assembly-anim-v2] [alias: --assignment-policy]
  -n, --no-anim
          Use skani only, without MUMmer4 alignment rescue
  -i, --input-kind <INPUT_KIND>
          Kind of input; assembly FASTA is the supported kind [default: assembly]
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

Uses skani plus MUMmer4 rescue by default. Use -n/--no-anim for skani only.
Guide: https://github.com/mbhall88/terobacter/blob/main/docs/assembly.md
```
