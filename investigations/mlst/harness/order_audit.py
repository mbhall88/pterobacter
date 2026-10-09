#!/usr/bin/env python3
"""Audit the report-order rule route D's tie-breaking relies on, from saved comparator output (no alignment is run).

    order_audit.py COMPARATOR_DIR OUT_TSV
Rule under test: within one contig and locus, the comparator's `--debug` hit rows list subjects (alleles) by best ungapped HSP score descending,
and equal-scoring subjects by allele id descending. For each fixture, contig and locus, subjects are taken in first-appearance order with
their best HSP score (nident - 2*(alen - nident), the +1/-2 ungapped score); every adjacent pair is checked. Ties between loci are
not part of the rule and are not counted. Allele id stands in for the BLAST OID, which follows file order (ids ascend in the package).
"""
import csv
import json
import sys
from pathlib import Path


def main(comp, out):
    rows, pairs, violations = [], 0, []
    for d in sorted(Path(comp).iterdir()):
        if d.name.startswith("invalid_") or not (d / "parsed.json").exists():  # invalid_duplicate_ids repeats a contig id, so contigs cannot be told apart
            continue
        hits = json.loads((d / "parsed.json").read_text())["hits"]
        per_locus = {}
        for h in sorted(hits, key=lambda x: x["order"]):
            score = h["nident"] - 2 * (h["alen"] - h["nident"])
            subj = per_locus.setdefault((h["contig"], h["locus"]), {})
            if h["allele"] not in subj:
                subj[h["allele"]] = score
            subj[h["allele"]] = max(subj[h["allele"]], score)
        n = bad = 0
        for (contig, locus), subj in per_locus.items():
            seq = list(subj.items())
            for (a1, s1), (a2, s2) in zip(seq, seq[1:]):
                n += 1
                if not (s1 > s2 or (s1 == s2 and int(a1) > int(a2))):
                    bad += 1
                    violations.append((d.name, contig, locus, a1, s1, a2, s2))
        pairs += n
        rows.append((d.name, n, bad))
    with open(out, "w", newline="") as fh:
        w = csv.writer(fh, delimiter="\t")
        w.writerow(["fixture", "adjacent_subject_pairs", "violations"])
        w.writerows(rows)
        w.writerow(["TOTAL", pairs, len(violations)])
    print(f"{pairs} adjacent within-locus subject pairs, {len(violations)} violations")
    for v in violations[:10]:
        print(" ", v)


if __name__ == "__main__":
    main(*sys.argv[1:3])
