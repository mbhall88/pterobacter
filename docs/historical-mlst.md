# Add multilocus sequence typing (MLST)

MLST reports the sequence type (ST) from the scheme's allele profile. Here, "historical" describes the bundled data cutoff, not a different typing method: the scheme retains submissions through 31 December 2024, retrieved in 2026. It is not a live copy of PubMLST.

Request sequence typing only if you need it alongside classification. Obtain the historical scheme directory and its trusted manifest digest through the private handoff.

```sh
ptero --reference-package "$REFERENCE_PACKAGE" --expected-sha256 "$REFERENCE_SHA256"   --mlst-package mlst --mlst-expected-sha256 "$MLST_SHA256"   --output typed-isolate.json --report-dir typed-isolate-report isolate.fa
```

Set `MLST_SHA256` to the supplied SHA-256 of `mlst/manifest.json`, and use your actual scheme path. MLST runs on the same whole input as classification and never changes the species, subspecies or genomovar result. No separate MLST or BLAST executable is required.

Inspect the `mlst` section of the JSON for the profile, allele states, dates and limitations. The scheme is a 2026 retrieval of PubMLST submissions through 31 December 2024. It is not a byte snapshot of that date, complete historical coverage has not been established, and it does not claim post-2024 currency. An unlisted profile does not establish a new ST; inexact, missing or ambiguous alleles remain explicit.

If requested typing fails after classification completes, the classification result is retained and the command exits 6. Check both results. Without these options, typing is not requested and the scheme package is unnecessary. [The command reference](assembly.md#optional-historical-mlst) lists exact MLST states; the [method contract](contracts/MLST-UNGAPPED-r1.md) describes the matcher.
