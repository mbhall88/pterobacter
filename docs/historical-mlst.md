# Add multilocus sequence typing (MLST)

MLST reports the sequence type (ST) from the scheme's allele profile. Here, "historical" describes the bundled data cutoff, not a different typing method: the scheme retains submissions through 31 December 2024, retrieved in 2026. It is not a live copy of PubMLST.

MLST is off by default. Install the historical scheme and request it when classifying:

```sh
ptero database install 2026-10-15 --mlst
ptero --mlst --report-dir typed-report isolate.fa
```

This scheme contains records submitted on or before 31 December 2024; it is not a current scheme. If you already have a verified scheme directory and its trusted manifest digest, you can use explicit paths instead:

```sh
ptero --reference-package "$REFERENCE_PACKAGE" --expected-sha256 "$REFERENCE_SHA256" \
  --anim-reference-package "$ANIM_REFERENCE_PACKAGE" --expected-anim-sha256 "$ANIM_SHA256" \
  --anim-output typed-alignments \
  --mlst-package mlst --mlst-expected-sha256 "$MLST_SHA256" \
  --output typed-isolate.json --report-dir typed-isolate-report isolate.fa
```

The command uses the default rescue mode and the alignment variables from [reference setup](reference-setup.md). To run skani only, use `--no-anim` and omit the three alignment options.

Set `MLST_SHA256` to the supplied SHA-256 of `mlst/manifest.json`, and use your actual scheme path. MLST runs on the same whole input as classification and never changes the species, subspecies or genomovar result. No separate MLST or BLAST executable is required.

Inspect the `mlst` section of the JSON for the profile, allele states, dates and limitations. The scheme is a 2026 retrieval of PubMLST submissions through 31 December 2024. It is not a byte snapshot of that date, complete historical coverage has not been established, and it does not claim post-2024 currency. An unlisted profile does not establish a new ST; inexact, missing or ambiguous alleles remain explicit.

If requested typing fails after classification completes, the classification result is retained and the command exits 6. Check both results. Without these options, typing is not requested and the scheme package is unnecessary. [The command reference](assembly.md#optional-historical-mlst) lists exact MLST states; the implementation contract at `docs/contracts/MLST-UNGAPPED-r1.md` describes the matcher.


## Using a newer Enterobacter scheme

The bundled package retains the historical cutoff described above. Do not edit that date to make an incompatible scheme load. A fresh PubMLST export needs packaging and verification before it can be used.

The [PubMLST Enterobacter page](https://pubmlst.org/organisms/enterobacter-spp) links to the typing database. The [current/custom package contract](contracts/MLST-PACKAGE-v2.md) supports separately versioned local seven-locus schemes with recorded provenance and rights. Automatic provisioning of a current PubMLST scheme is not available; source access and redistribution terms still need review. Updating a scheme does not change species, subspecies or genomovar rules.
