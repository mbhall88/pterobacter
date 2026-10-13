# Set up the reference data

Obtain a reference directory and the trusted SHA-256 of its `portable.json` from the private package handoff. The executable does not download a database. No public data URL is currently available.

Keep the directory intact. Before analysis, the program checks the manifest against the supplied digest, then verifies its listed files and metadata. It checks the package again before publishing results. A checksum computed from an untrusted download does not establish the package's origin.

## Use the supplied directory

Set these shell variables to the path and digest in your trusted handoff:

```sh
REFERENCE_PACKAGE=/path/to/reference
REFERENCE_SHA256=THE_TRUSTED_PORTABLE_JSON_SHA256
```

Replace both placeholders. In the current interface these variables are just shell conveniences; every command still receives the directory and digest explicitly:

```sh
tero --reference-package "$REFERENCE_PACKAGE" --expected-sha256 "$REFERENCE_SHA256"   --output isolate.json --report-dir isolate-report isolate.fa
```

Reuse the same package for another assembly or batch. Do not move output files into the reference directory. The [coverage guide](supported-taxa.md) describes which names and groups it represents.

## Optional components

The default analysis uses skani and the main reference package. To request the candidate ANIm rescue workflow, you also need the supplied whole-assembly reference FASTA directory and its trusted `anim.json` digest when alignments are required. [The command reference](assembly.md#candidate-v2-anim-rescue) gives the complete invocation. It keeps the original calling rules and reports the candidate validation status; changing the policy is an explicit choice.

Historical MLST uses its own directory and trusted `manifest.json` digest. Follow [historical MLST setup](historical-mlst.md). It adds typing information without changing classification.

Once the executable and required components are provisioned, classification is offline. A future one-time configuration command is under discussion; it is not part of this interface.
