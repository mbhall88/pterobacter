# Add multilocus sequence typing (MLST)

MLST reports the sequence type (ST) from the scheme's allele profile. Here, "historical" describes the bundled data cutoff, not a different typing method: the scheme retains submissions through 31 December 2024, retrieved in 2026. It is not a live copy of PubMLST.

MLST is off by default in the current build. To include a sequence type alongside classification, obtain the historical scheme directory and its trusted manifest digest through the private handoff, then pass both options:

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

The current loader accepts only the historical package described above. It checks the 31 December 2024 submission cutoff and its limitation text, as well as the allele/profile hashes. A fresh PubMLST export cannot be passed directly to `--mlst-package`, and editing the cutoff to make it load would misdescribe the data.

The [PubMLST Enterobacter page](https://pubmlst.org/organisms/enterobacter-spp) links to the typing database; PubMLST also documents [allele/profile downloads and API access](https://pubmlst.org/data). Those sources provide scheme data, which still need a compatible package before this executable can use them.

[Managed database support](https://github.com/mbhall88/pterobacter/issues/39) tracks selecting an installed MLST scheme without a package path and importing a separately versioned newer scheme. That change will include a tested download/install/classify example with the scheme's retrieval date and hashes. Until it is implemented, the command above uses the historical package. Updating the scheme will not change the species, subspecies or genomovar rules.
