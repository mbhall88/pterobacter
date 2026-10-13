# Interpret a genomovar match

Look for a genomovar only after an eligible species and subspecies call. In this reference package the definitions cover the proposed *Enterobacter xiangfangensis* subsp. *hoffmannii*: 25 informal chromosome ANI groups built from 173 historical chromosome members. They are not formal taxonomic names.

A match requires sufficient ANI and coverage to every member of exactly one definition, with measured evidence excluding the alternatives. The query remains the whole assembly as supplied; the reference definition uses historical chromosome record sets. A missing rival comparison can block the match.

A unique result means compatibility with the supplied definition. An unrepresented group can match a neighbouring group, so the result does not prove that the query's source group is present in the database. Two compatible groups are withheld, rather than choosing the nearest one. Fifteen of the definitions have only one member; matching one carries a sparse-reference warning and does not establish its population boundary.

Read the `genomovar` section for the result, reasons, warnings and member comparisons. A withheld genomovar does not by itself erase a supported species or subspecies call. A candidate ANIm species rescue supports neither subspecies nor genomovar; custom thresholds do not support a genomovar either.

Two queries with the same result are not necessarily at least 99.5% ANI to each other. ST does not break a genomovar tie. For exact gates, reason codes and retained evidence, see [the genomovar reference](genomovar.md). For deeper investigation, use [boundary review](boundary-review.md).
