# Set up the reference data

Use [database management](databases.md) to install a trusted version once, then select it when classifying assemblies. The installer accepts local archives and supports catalogue-pinned HTTPS downloads. No public catalogue or download URL is available yet; the guide includes the supported local installation workflow.

If you already have unpacked reference directories and their trusted manifest hashes, you can use them directly as shown below.

Keep the directory intact. Before analysis, the program checks the manifest against the supplied digest, then verifies its listed files and metadata. It checks the package again before publishing results. A checksum computed from an untrusted download does not establish the package's origin.

## Use the supplied directory

Set these shell variables to the path and digest in your trusted handoff:

```sh
REFERENCE_PACKAGE=/path/to/reference
REFERENCE_SHA256=THE_TRUSTED_PORTABLE_JSON_SHA256
ANIM_REFERENCE_PACKAGE=/path/to/reference-fastas
ANIM_SHA256=THE_TRUSTED_ANIM_JSON_SHA256
```

Replace these placeholders with the two directories and their supplied digests. These variables are shell conveniences for the explicit-directory workflow; pass them with each invocation:

```sh
ptero --reference-package "$REFERENCE_PACKAGE" --expected-sha256 "$REFERENCE_SHA256" \
  --anim-reference-package "$ANIM_REFERENCE_PACKAGE" --expected-anim-sha256 "$ANIM_SHA256" \
  --anim-output isolate-alignments \
  --output isolate.json --report-dir isolate-report isolate.fa
```

Reuse the same package for another assembly or batch. Do not move output files into the reference directory. The [coverage guide](supported-taxa.md) describes which names and groups it represents.

## Alignment references and MLST

The default analysis starts with skani and adds MUMmer4 alignments for eligible borderline queries. Supply the matching whole-assembly reference FASTA directory and its trusted `anim.json` digest so those comparisons can run. [The command reference](assembly.md#boundary-anim) gives the complete invocation. A query that needs alignments fails with an incomplete report if this component is absent; it never silently switches to skani-only classification. To choose skani only, add `--no-anim` and omit `--anim-reference-package`, `--expected-anim-sha256` and `--anim-output`.

For a managed database, install the optional historical MLST component and request typing with `--mlst`. An explicit scheme directory and trusted `manifest.json` digest are also supported. Follow [historical MLST setup](historical-mlst.md). Typing does not change classification.

Once the executable and required components are installed, classification is offline. Use `ptero database list` to inspect installed versions and `ptero database verify VERSION` to recheck their contents.
