# Set up the reference data

Obtain a reference directory and the trusted SHA-256 of its `portable.json` from the private package handoff. The executable does not download a database. No public data URL is currently available.

Keep the directory intact. Before analysis, the program checks the manifest against the supplied digest, then verifies its listed files and metadata. It checks the package again before publishing results. A checksum computed from an untrusted download does not establish the package's origin.

## Use the supplied directory

Set these shell variables to the path and digest in your trusted handoff:

```sh
REFERENCE_PACKAGE=/path/to/reference
REFERENCE_SHA256=THE_TRUSTED_PORTABLE_JSON_SHA256
ANIM_REFERENCE_PACKAGE=/path/to/reference-fastas
ANIM_SHA256=THE_TRUSTED_ANIM_JSON_SHA256
```

Replace these placeholders with the two directories and their supplied digests. In the current interface these variables are just shell conveniences; every command still receives the directory and digest explicitly:

```sh
tero --reference-package "$REFERENCE_PACKAGE" --expected-sha256 "$REFERENCE_SHA256" \
  --anim-reference-package "$ANIM_REFERENCE_PACKAGE" --expected-anim-sha256 "$ANIM_SHA256" \
  --anim-output isolate-alignments \
  --output isolate.json --report-dir isolate-report isolate.fa
```

Reuse the same package for another assembly or batch. Do not move output files into the reference directory. The [coverage guide](supported-taxa.md) describes which names and groups it represents.

## Alignment references and MLST

The default analysis starts with skani and adds MUMmer4 alignments for eligible borderline queries. Supply the matching whole-assembly reference FASTA directory and its trusted `anim.json` digest so those comparisons can run. [The command reference](assembly.md#boundary-anim) gives the complete invocation. A query that needs alignments fails with an incomplete report if this component is absent; it never silently switches to skani-only classification. To choose skani only, add `--no-anim` and omit `--anim-reference-package`, `--expected-anim-sha256` and `--anim-output`.

Historical MLST uses its own directory and trusted `manifest.json` digest. Follow [historical MLST setup](historical-mlst.md). It adds typing information without changing classification.

Once the executable and required components are provisioned, classification is offline. A future one-time configuration command is under discussion; it is not part of this interface.
