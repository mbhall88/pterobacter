#!/usr/bin/env python3
"""Compare one candidate run against the comparator run, per fixture.

    compare.py PACKAGE_PUBMLST_DIR COMPARATOR_DIR CANDIDATE_DIR OUT_PREFIX
Parity = identical tokens, ST, STATUS, SCORE, importer-level state, candidate ST and exact-hit locations.
Hit-level agreement (locus, allele, contig, start, end, strand, nident, alen over hits that pass mincov 50%) is
reported separately.
"""
import csv
import json
import re
import sys
from pathlib import Path

LOCI = ["dnaA", "fusA", "gyrB", "leuS", "pyrG", "rplB", "rpoB"]


def load_package(pkg):
    alleles = {}
    for l in LOCI:
        cur = None
        for line in (Path(pkg) / f"{l}.tfa").read_text().splitlines():
            if line.startswith(">"):
                cur = re.split(r"[_-]", line[1:].split()[0])[-1]
                alleles[(l, cur)] = []
            else:
                alleles[(l, cur)].append(line.strip().upper())
    alleles = {k: "".join(v) for k, v in alleles.items()}
    profiles = {}
    for line in (Path(pkg) / "ecloacae.txt").read_text().splitlines()[1:]:
        c = line.split("\t")
        profiles[tuple(c[1:8])] = c[0]
    return alleles, profiles


def derive(tokens, exact_locs, alleles, profiles):
    """Importer-level state and profile-compatible candidate ST (mirrors the Enterobacter importer)."""
    codes = [tokens[l] for l in LOCI]
    if any("," in c for c in codes):
        state = "ambiguous_alleles"
    elif "-" in codes:
        state = "incomplete_profile"
    elif any("~" in c or "?" in c for c in codes):
        state = "inexact_alleles"
    else:
        state = "exact_profile_match" if tuple(codes) in profiles else "unmatched_profile"
    repeated, unconfirmed, signature = [], [], []
    for l in LOCI:
        code = tokens[l]
        exact = re.fullmatch(r"[0-9]+(?:,[0-9]+)*", code) is not None
        ids = set(code.split(",")) if exact else set()
        signature.append(next(iter(ids)) if len(ids) == 1 else None)
        if "," in code:
            seqs = {alleles[(l, a)] for a in ids}
            locs = {(x[2], x[3], x[4]) for x in exact_locs if x[0] == l}
            if len(seqs) == 1 and len(ids) == 1:  # one allele repeated: copy-supported only at distinct locations
                (repeated if len(locs) > 1 else unconfirmed).append(l)
    cand = profiles.get(tuple(signature)) if repeated and not unconfirmed and all(s is not None for s in signature) else None
    return state, cand


def read_comparator(d):
    p = json.loads((d / "parsed.json").read_text())
    hits = set()
    exact = set()
    for h in p["hits"]:
        if h["nident"] / h["hlen"] >= 0.5:
            hits.add((h["locus"], h["allele"], h["contig"], h["start"], h["end"], h["strand"], h["nident"], h["alen"]))
            if h["alen"] == h["hlen"] == h["nident"]:
                exact.add((h["locus"], h["allele"], h["contig"], h["start"], h["end"], h["strand"]))
    return dict(tokens=p["tokens"], ST=p["ST"], status=p["status"], score=p["score"]), hits, exact


def read_candidate(d):
    r = json.loads((d / "result.json").read_text())
    c = r["call"]
    hits = set()
    for row in csv.DictReader((d / "hits.tsv").open(), delimiter="\t"):
        if int(row["nident"]) / int(row["hlen"]) >= 0.5:
            hits.add((row["locus"], row["allele"], row["contig"], int(row["start"]), int(row["end"]), row["strand"], int(row["nident"]), int(row["alen"])))
    exact = {(e["locus"], e["allele"], e["contig"], e["start"], e["end"], e["strand"]) for e in c["exact_locations"]}
    return dict(tokens=dict(zip(LOCI, c["tokens"])), ST=c["ST"], status=c["status"], score=c["score"]), hits, exact, r


def main(pkg, comp, cand, prefix):
    alleles, profiles = load_package(pkg)
    comp, cand = Path(comp), Path(cand)
    rows = []
    for d in sorted(p for p in cand.iterdir() if p.is_dir()):
        name = d.name
        rec = json.loads((d / "record.json").read_text())
        row = dict(fixture=name, candidate_exit=rec["exit_code"], wall_s=rec["wall_seconds"], rss_mib=round((rec["max_rss_kib"] or 0) / 1024, 1))
        if name.startswith("invalid_"):
            row.update(kind="invalid", comparator_exit=json.loads((comp / name / "record.json").read_text())["exit_code"] if (comp / name / "record.json").exists() else None)
            rows.append(row)
            continue
        c_call, c_hits, c_exact = read_comparator(comp / name)
        if rec["exit_code"] != 0:
            row.update(kind="valid", parity="CANDIDATE_FAILED")
            rows.append(row)
            continue
        k_call, k_hits, k_exact, raw = read_candidate(d)
        c_state, c_cand = derive(c_call["tokens"], c_exact_to_list(c_exact), alleles, profiles)
        k_state, k_cand = derive(k_call["tokens"], c_exact_to_list(k_exact), alleles, profiles)
        diffs = []
        for l in LOCI:
            if c_call["tokens"][l] != k_call["tokens"][l]:
                diffs.append(f"{l}: comparator {c_call['tokens'][l]} vs candidate {k_call['tokens'][l]}")
        for f in ("ST", "status", "score"):
            if c_call[f] != k_call[f]:
                diffs.append(f"{f}: {c_call[f]} vs {k_call[f]}")
        if c_state != k_state:
            diffs.append(f"state: {c_state} vs {k_state}")
        if c_cand != k_cand:
            diffs.append(f"candidate_ST: {c_cand} vs {k_cand}")
        if c_exact != k_exact:
            diffs.append(f"exact locations: comparator-only {len(c_exact - k_exact)}, candidate-only {len(k_exact - c_exact)}")
        row.update(kind="valid", parity="PASS" if not diffs else "FAIL", differences="; ".join(diffs),
                   comparator=f"{''.join(c_call['tokens'][l]+' ' for l in LOCI).strip()} ST={c_call['ST']} {c_call['status']} {c_call['score']}",
                   hits_comparator=len(c_hits), hits_candidate=len(k_hits), hits_shared=len(c_hits & k_hits),
                   hits_missing_in_candidate=len(c_hits - k_hits), hits_extra_in_candidate=len(k_hits - c_hits),
                   comparator_state=c_state, comparator_candidate_ST=c_cand, search_s=round(raw["seconds"]["search"], 3))
        rows.append(row)
    Path(prefix + ".json").write_text(json.dumps(rows, indent=1) + "\n")
    cols = ["fixture", "kind", "parity", "differences", "comparator", "hits_comparator", "hits_candidate", "hits_shared", "hits_missing_in_candidate",
            "hits_extra_in_candidate", "candidate_exit", "comparator_exit", "wall_s", "rss_mib"]
    with open(prefix + ".tsv", "w") as fh:
        w = csv.DictWriter(fh, cols, delimiter="\t", extrasaction="ignore")
        w.writeheader()
        w.writerows(rows)
    valid = [r for r in rows if r["kind"] == "valid"]
    print(f"{sum(r.get('parity') == 'PASS' for r in valid)}/{len(valid)} valid fixtures at parity")
    for r in valid:
        if r.get("parity") != "PASS":
            print(" ", r["fixture"], r.get("parity"), r.get("differences", ""))


def c_exact_to_list(exact):
    return [(e[0], e[1], e[2], e[3], e[4]) for e in exact]


if __name__ == "__main__":
    main(*sys.argv[1:5])
