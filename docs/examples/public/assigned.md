# Assigned genome: RHBSTW-00333, ST286

[GCA_013738375.1](https://www.ncbi.nlm.nih.gov/datasets/genome/GCA_013738375.1/) is a public whole assembly with a chromosome and seven plasmids. The [reproduction script](reproduce.sh) supplies all eight records.

| Result | Value |
|---|---|
| Species | *Enterobacter xiangfangensis* |
| Subspecies | *E. xiangfangensis* subsp. *hoffmannii* — proposed combination |
| MLST scheme | `ecloacae` |
| Sequence type | **ST286** |
| MLST state | `exact_profile_match` |
| Genomovar | `ECTX-GV-2026-10-02-v1-GV10` |
| ANIm | Not triggered: skani already supports the call |

The seven MLST allele IDs are:

| dnaA | fusA | gyrB | leuS | pyrG | rplB | rpoB |
|---|---|---|---|---|---|---|
| 59 | 64 | 81 | 9 | 79 | 37 | 38 |

An excerpt selected from the actual `mlst` object:

```json
{
  "scheme": "ecloacae",
  "sequence_type": "286",
  "state": "exact_profile_match",
  "status": "complete",
  "alleles": {
    "dnaA": "59", "fusA": "64", "gyrB": "81", "leuS": "9",
    "pyrG": "79", "rplB": "37", "rpoB": "38"
  }
}
```

This assembly is included among the database's diversity references and genomovar members. Its strongest species reference is therefore itself: 100% skani ANI, with 99.99% aligned fraction on each side. That makes it a useful installation and output example, not a test of generalisation to an unseen genome. The GV10 call also checks compatibility with the other members of its definition; it is an operational match, not proof of a population boundary.

The database's proposed subspecies name differs from NCBI's *E. hormaechei* label. [Supported taxa](../../supported-taxa.md) explains the naming framework. MLST remains a separate result and does not determine that name.

[Full readable report](single/report.md) · [JSON](single/report.json) · [Calls TSV](single/calls.tsv) · [Evidence TSV](single/evidence.tsv) · [Download and run](../README.md#run-them-yourself)
