# Example outputs

These synthetic examples preserve the original JSON and TSV files. The readable summaries are rendered with the current labels; their calls and evidence tables match the originals exactly. They are not biological measurements. Fixture software/data versions and names belong to the tests and do not identify the current release.

## One assembly

[JSON](output/single/report.json) · [Calls TSV](output/single/calls.tsv) · [Evidence TSV](output/single/evidence.tsv) · [Readable Markdown](readable/single.md)

These files were rendered from the assigned-assembly fixture in [`tests/report.rs`](../../tests/report.rs). The species and subspecies are assigned and the genomovar is withheld.

## Batch outputs

[JSON array](output/batch/report.json) · [Calls TSV](output/batch/calls.tsv) · [Evidence TSV](output/batch/evidence.tsv) · [Readable Markdown](readable/batch.md)

The batch combines a skani assignment, an input failure and an ANIm-supported assignment from the same fixture. This deliberately mixes outcomes with and without ANIm to illustrate reporting; one normal invocation uses the same ANIm setting for all inputs. A failed input keeps its own execution status and does not erase the other rows.

## Genomovar match

The [assigned-genomovar excerpt](output/genomovar-excerpt.json) selects `status`, `genomovar_id` and `reasons` from `Case::new().run()` in [`tests/genomovar_policy.rs`](../../tests/genomovar_policy.rs). Its small synthetic definition uses the short ID `GV01`; production definitions use versioned concept IDs. This is a decision-function fixture, not a whole-assembly run.

## Sources

The [original JSON excerpt](synthetic-report-excerpt.json) remains the exact selection from the assigned-assembly fixture. The [recorded public-assembly example](../boundary-review.md#a-recorded-wider-context-example) supplies observed comparisons and an expert tree review separately.

Original fixture and renderer SHA-256 hashes, retained for the historical files:

- `tests/report.rs`: `f1aebab28783eec31617654f9c64b44b7f7a0101d16512b3081e65b4021cfc4f`
- `tests/genomovar_policy.rs`: `bc5302ea1fd09df79151ecb855b8e24c1e4ffd83bf03049b61acee1e4de9ec71`
- `src/report.rs`: `e47fe20df47033a0dd5ef61372e177cd1c47d60df2f9771440161467b8477f38`
- `src/genomovar.rs`: `9e2eb08b8413bb507b265a261fa74ee3fd9a194af559a0166156ffd0136c88e1`
