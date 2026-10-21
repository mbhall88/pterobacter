# Multilocus sequence typing (MLST)

MLST reports the sequence type (ST) from the scheme's allele profile. The [ST286 example](examples/public/assigned.md) shows a complete allele profile; the [ST2274 example](examples/public/unresolved.md) shows why an ST can be reported even when species is unresolved. It is included in database installation and runs by default:

```sh
ptero database install 2026-10-15
ptero --report-dir typed-report isolate.fa
```

To skip typing, pass `-x` or `--no-mlst` to classification. The same option on `database install` omits the scheme download:

```sh
ptero database install 2026-10-15 --no-mlst
ptero --no-mlst --report-dir untyped-report isolate.fa
```

The bundled scheme covers submissions through **31 December 2024**, retrieved in 2026. It is not a live copy of PubMLST.

If you already have a verified scheme directory and its trusted manifest digest, you can use explicit paths instead:

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

If requested typing fails after classification completes, the classification result is retained and the command exits 6. Check both results. With `--no-mlst`, typing is not requested and the scheme package is neither needed nor opened. When using explicit reference paths, supply an MLST package too, or opt out with `--no-mlst`. [The command reference](assembly.md#mlst) lists exact MLST states; the implementation contract at `docs/contracts/MLST-UNGAPPED-r1.md` describes the matcher.


## Using a newer Enterobacter scheme

The bundled package retains the submission cutoff described above. Do not edit that date to make an incompatible scheme load. A fresh PubMLST export needs packaging and verification before it can be used.

The [PubMLST Enterobacter page](https://pubmlst.org/organisms/enterobacter-spp) links to the typing database. The [current/custom package contract](contracts/MLST-PACKAGE-v2.md) supports separately versioned local seven-locus schemes with recorded provenance and rights. Automatic provisioning of a current PubMLST scheme is not available; source access and redistribution terms still need review. Updating a scheme does not change species, subspecies or genomovar rules.
