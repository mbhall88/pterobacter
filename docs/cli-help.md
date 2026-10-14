# Command-line help

For a first run, follow [classify an assembly](running.md). This page explains the less familiar options, followed by the exact help printed by the current executable.

## Classification mode and ANI thresholds

The current default uses **skani only**. The optional **skani plus MUMmer4 rescue** mode checks some uncertain species calls with whole-genome alignments. The internal names `conservative-assembly-v1` and `conservative-assembly-anim-v2` identify these calling rules; they are not software release numbers. See [the comparison and selection command](assembly.md#classification-modes).

`--species-ani` and `--subspecies-ani` change the minimum average nucleotide identity (ANI) percentages for species and subspecies calls. They are advanced options for the rescue mode, with defaults of 96 and 98 respectively. Supplying either option marks the result `custom_unvalidated`, even if you supply the default number. Leave them out to use the standard rules. [Threshold details](v2-decisions.md#custom-ani-thresholds) explain which comparisons and safeguards apply.

Assemblies are already the only supported input. Omit `--input-kind`; it defaults to `assembly` and does not enable read classification. [Issue43](https://github.com/mbhall88/terobacter/issues/43) tracks simplifying this interface and the terminal help.

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

Captured from `tero --help` at source commit `63bd432`. The `terobacter` alias accepts the same options. The descriptions above explain the internal terms that still appear in this output.

```text
Standalone, self-contained Enterobacter isolate classification

Usage: tero [OPTIONS] --reference-package <REFERENCE_PACKAGE> --expected-sha256 <EXPECTED_SHA256> <ASSEMBLY>...

Arguments:
  <ASSEMBLY>...  Isolate assembly FASTA files, analysed whole and as supplied

Options:
      --species-ani <T>
          Custom v2 species ANI percentage; always marks results custom_unvalidated
      --subspecies-ani <S>
          Custom v2 subspecies ANI percentage; requires 80 <= T <= S <= 100
      --reference-package <REFERENCE_PACKAGE>
          Authenticated reference package directory
      --expected-sha256 <EXPECTED_SHA256>
          Expected SHA-256 of portable.json, obtained from a trusted source
      --policy <POLICY>
          Native classification policy; v1 or conservative-assembly-anim-v2 [default: conservative-assembly-v1] [alias: --assignment-policy]
      --input-kind <INPUT_KIND>
          Kind of input; assembly FASTA is the supported kind [default: assembly]
      --threads <THREADS>
          Worker threads for each assembly [default: 1]
      --jobs <JOBS>
          Assemblies analysed at the same time, each with its own --threads [default: 1]
      --output <FILE>
          Write the JSON report to this new file instead of standard output; never replaced
      --mlst-package <MLST_PACKAGE>
          Historical MLST scheme package directory; requests optional supplementary MLST
      --mlst-expected-sha256 <MLST_EXPECTED_SHA256>
          Expected SHA-256 of the MLST package manifest.json, obtained from a trusted source
      --report-dir <DIR>
          Write report.json, calls.tsv, evidence.tsv and report.md into this new directory
      --anim-reference-package <ANIM_REFERENCE_PACKAGE>
          Optional separate whole-assembly reference FASTA component
      --expected-anim-sha256 <EXPECTED_ANIM_SHA256>
          Trusted SHA-256 of the component's anim.json
      --anim-output <ANIM_OUTPUT>
          Fresh directory retaining native evidence; must not already exist
      --anim-cache <ANIM_CACHE>
          Directory of verified reciprocal pairs to reuse and extend; created when absent
      --anim-pair-seconds <ANIM_PAIR_SECONDS>
          Reciprocal pair wall timeout, default 120 seconds, maximum 1800
      --anim-query-seconds <ANIM_QUERY_SECONDS>
          Total ANIm wall budget per query, default 600 seconds, maximum 7200
      --anim-memory-mib <ANIM_MEMORY_MIB>
          Native worker memory ceiling, default 1024 MiB, maximum 6144 MiB
  -h, --help
          Print help
  -V, --version
          Print version
```
