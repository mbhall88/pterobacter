# P4a scorer fixtures

`scoring-cases.json` holds the 84 scoring cases of the accepted P4a-r3 evaluation design, copied byte for byte: 40 rescue records, 4 rescue records the scorer must refuse, 26 genomovar records, 4 summary rows, 1 species cluster and 9 interval checks. The records are synthetic scorer inputs, not genomes or classifier output.

| Source | SHA-256 |
|---|---|
| P4a-r3 scoring cases | `9a93bf22072bcdf5de7664bf1fc7e34311582491b369fcfdcfd57b33427c5720` |
| P4a-r3 evaluation design | `4f9357d01fe05d38a6800d615f1cd7a8a13c515c40dcac3cc41998dc54da9247` |
| P4a-r3 reference scorer | `a3bab5d8824cf91d201b640850a7f24d8e2867d694f378524046fef18a4377d3` |

`tests/p4a_scorer_contract.rs` checks the first pin, then checks that every case is accounted for against terobacter's output. Every contract reason in a rescue or genomovar record must map from a terobacter reason through [reason-vocabulary.tsv](../../../docs/contracts/reason-vocabulary.tsv), and every field the scorer reads must have a source in [scorer-fields.tsv](../../../docs/contracts/scorer-fields.tsv). Three reason names are not contract reasons and are exempt: the placeholders `a` and `b` in R35, and `mystery` in G20, which the scorer must reject as `unknown_genomovar_reason`. The test also checks that each mapped output path exists in real decision, plan and diagnostic output.

The scorer itself stays with the design in the research workspace. These tests show that terobacter's output can be mapped onto what it reads. They do not run it and say nothing about accuracy.
