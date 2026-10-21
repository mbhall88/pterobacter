# Unresolved genome: ECC-219, ST2274

[GCA_043966805.1](https://www.ncbi.nlm.nih.gov/datasets/genome/GCA_043966805.1/) is a public chromosome assembly labelled *Enterobacter* sp. ECC-219. It is the second row in the [batch report](batch/report.md).

| Result | Value |
|---|---|
| Species | **Unresolved** |
| Subspecies / genomovar | Withheld |
| MLST | **ST2274**, `exact_profile_match` |
| Closest species reference | *E. nematophilus* type anchor, GCA_026344075.1 |
| skani ANI | 94.83% |
| skani aligned fraction | 82.65% query; 88.26% reference |
| Reciprocal ANIm | 95.02965% and 95.03431% |
| Lowest ANIm aligned fraction | 84.94% |
| Reason | `ANIm_no_strict_witness` |

The genome falls in the range where the tool runs ANIm. That comparison finishes successfully, but neither orientation exceeds the required 96% ANI. The species stays unresolved despite adequate aligned fraction. Empty species fields here are intentional, not missing output or an execution failure.

MLST still finds the complete seven-locus profile:

| dnaA | fusA | gyrB | leuS | pyrG | rplB | rpoB |
|---|---|---|---|---|---|---|
| 590 | 263 | 415 | 493 | 302 | 170 | 101 |

The full JSON contains `mlst.sequence_type: "2274"` alongside `species_name: null`. An ST is an allele-profile identifier; it does not establish species membership. Do not copy *nematophilus* from the nearest-reference table into the assigned-species field.

To investigate further, follow [boundary review](../../boundary-review.md). Keep the original automatic report and record any subsequent expert interpretation separately.

[Full batch JSON](batch/report.json) · [Calls TSV](batch/calls.tsv) · [Evidence TSV](batch/evidence.tsv) · [Download and run](../README.md#run-them-yourself)
