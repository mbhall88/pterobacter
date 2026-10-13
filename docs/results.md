# Read your results

Open `report.md` in the directory you supplied with `--report-dir`. Start with the final call, reasons and warnings, then examine the supporting comparisons. Use `calls.tsv` for a batch overview and JSON for a pipeline.

## Find the answer

| File | Use |
|---|---|
| `report.md` | Read the result, warnings and comparison tables. |
| `calls.tsv` | One row per assembly with final calls and reasons. |
| `report.json` | Full authoritative report, including versions and provenance. |
| `evidence.tsv` | Supporting reference comparisons and challenges. |

The JSON sent to standard output or `--output FILE` contains the same report as `report.json`. TSV and Markdown are derived from it. One input produces an object; a batch produces an array.

The [small JSON excerpt](examples/synthetic-report-excerpt.json) shown in the README is taken from the existing synthetic report fixture described in [the example notes](examples/README.md). Its species and subspecies are assigned, but its genomovar is withheld because that synthetic parent does not qualify for the genomovar definitions. It is an output-format example, not evidence of biological accuracy.

## Separate a call from its nearest reference

`calls.tsv` reports `assigned` when the rules support a species, `unresolved` when a completed run cannot support one, and `not_evaluated` when no decision was reached. A species can be assigned while subspecies or genomovar remains empty. Withheld assignment fields are null in JSON and empty in the TSV.

A nearest-reference name can still appear beside an unresolved result. Read it as comparison evidence, not as the assigned name. Reasons explain the withholding. Execution errors are separate: check `execution_status`, `execution_stage` and `execution_error` in the TSV, or `execution` in JSON. Some later-stage failures retain independently supported parent calls; inspect each rank and the failure rather than treating the whole row as a biological negative.

## Read the warnings

A genomovar match means compatibility with the supplied definition. It cannot prove that the query's source group is represented: an unrepresented group can match a neighbour. Overlapping compatible groups are withheld; one-member groups carry a sparse-reference warning. [Genomovar matches](genomovar-guide.md) explains these limits.

ANI and aligned fraction are percentages. An aligned fraction of `0.8` means 0.8%, while `80` means 80%. Missing comparisons are not zero ANI or evidence against an unmeasured rival. A clean report does not certify assembly purity.

For an uncertain call, follow [boundary review](boundary-review.md). The [report reference](report-reference.md) preserves exact field names, method-specific measurements, competitor completeness and version fields. The candidate ANIm workflow retains `not_yet_validated` in its output; this documentation does not change that status.
