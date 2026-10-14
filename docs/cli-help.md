# Command-line help

Captured from `tero --help` at source commit `63bd432`. The `terobacter` alias accepts the same options.

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
