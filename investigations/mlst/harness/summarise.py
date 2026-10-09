#!/usr/bin/env python3
"""Collect per-route parity, hit-level and performance tables from a scratch directory of runs.

    summarise.py SCRATCH_DIR OUT_DIR
SCRATCH_DIR holds fixtures/, comparator/, cmp_<route>.json (compare.py output), perf*.json and perf*.work/.
"""
import csv
import json
import re
import sys
from pathlib import Path

LOCI = ["dnaA", "fusA", "gyrB", "leuS", "pyrG", "rplB", "rpoB"]
ROUTES = [  # (label, description)
    ("ungapped", "D: exact 32-mer seeding + ungapped X-drop (no alignment library)"),
    ("mm2inmem_asm5", "1: minimap2 C binding, in-memory index, asm5"),
    ("mm2inmem_asm10", "1: minimap2 C binding, in-memory index, asm10"),
    ("mm2inmem_asm20", "1: minimap2 C binding, in-memory index, asm20"),
    ("mm2file_asm5", "1*: minimap2 C binding, file index, asm5 (supplementary)"),
    ("mm2file_asm10", "1*: minimap2 C binding, file index, asm10 (supplementary)"),
    ("mm2file_asm20", "1*: minimap2 C binding, file index, asm20 (supplementary)"),
    ("pure_asm5", "1b: minimap2-pure-rs, asm5"),
    ("pure_asm10", "1b: minimap2-pure-rs, asm10"),
    ("pure_asm20", "1b: minimap2-pure-rs, asm20"),
    ("exact-bio", "2: exact hash + seeds + rust-bio local alignment"),
    ("exact-ba", "2: exact hash + seeds + block-aligner X-drop extension"),
    ("exact-only", "2x: exact hash only (not parity by construction)"),
]


def main(scratch, out):
    S, O = Path(scratch), Path(out)
    O.mkdir(parents=True, exist_ok=True)
    manifest = json.loads((S / "fixtures" / "manifest.json").read_text())
    cmp = {r: json.loads((S / f"cmp_{r}.json").read_text()) for r, _ in ROUTES if (S / f"cmp_{r}.json").exists()}
    routes = [r for r, _ in ROUTES if r in cmp]
    # per fixture matrix
    with (O / "parity-per-fixture.tsv").open("w") as fh:
        w = csv.writer(fh, delimiter="\t")
        w.writerow(["fixture", "description", "comparator"] + routes)
        names = list(manifest["fixtures"])
        for n in names:
            row = [n, manifest["fixtures"][n]["description"], ""]
            for r in routes:
                rec = next(x for x in cmp[r] if x["fixture"] == n)
                row[2] = rec.get("comparator", row[2])
                row.append({"PASS": "PASS", "FAIL": "FAIL: " + rec.get("differences", ""), "CANDIDATE_FAILED": f"CRASH(exit {rec['candidate_exit']})"}[rec["parity"]])
            w.writerow(row)
    # summary
    with (O / "parity-summary.tsv").open("w") as fh:
        w = csv.writer(fh, delimiter="\t")
        w.writerow(["route", "description", "fixtures", "call_parity_pass", "fail", "crash", "hsps_comparator", "hsps_missing_in_candidate", "hsps_extra_in_candidate"])
        for r, d in ROUTES:
            if r not in cmp:
                continue
            v = [x for x in cmp[r] if x["kind"] == "valid"]
            w.writerow([r, d, len(v), sum(x["parity"] == "PASS" for x in v), sum(x["parity"] == "FAIL" for x in v), sum(x["parity"] == "CANDIDATE_FAILED" for x in v),
                        sum(x.get("hits_comparator", 0) for x in v), sum(x.get("hits_missing_in_candidate", 0) for x in v), sum(x.get("hits_extra_in_candidate", 0) for x in v)])
    # invalid inputs
    base = cmp["ungapped"]
    with (O / "invalid-inputs.tsv").open("w") as fh:
        w = csv.writer(fh, delimiter="\t")
        w.writerow(["input", "comparator_exit", "probe_exit", "note"])
        for x in base:
            if x["kind"] == "invalid":
                w.writerow([x["fixture"], x["comparator_exit"], x["candidate_exit"],
                            "comparator accepts" if x["comparator_exit"] == 0 else "comparator rejects"])
    # performance
    rows, seen = [], set()
    total_cpu = 0.0
    for f in sorted(S.glob("perf*.json")):
        for r in json.loads(f.read_text()):
            total_cpu += (r["user_seconds"] or 0) + (r["sys_seconds"] or 0)
            if (r["label"], r["input"]) not in seen:  # the comparator is re-run by each perf pass; keep the first
                seen.add((r["label"], r["input"]))
                rows.append(r)
    calls = {}
    def tok(path, label):
        txt = Path(path).read_text()
        if label == "comparator":
            line = txt.splitlines()[1].split("\t")
            t = dict(re.fullmatch(r"(\w+)\((.*)\)", x).groups() for x in line[5].split(";"))
            return ([t[l] for l in LOCI], line[2], line[3], int(line[4]))
        c = json.loads(txt)["call"]
        return (c["tokens"], c["ST"], c["status"], c["score"])
    for r in rows:
        calls[(r["label"], Path(r["input"]).stem)] = tok(r["stdout"], r["label"])
    labels = []
    for r in rows:
        if r["label"] not in labels:
            labels.append(r["label"])
    with (O / "performance.tsv").open("w") as fh:
        w = csv.writer(fh, delimiter="\t")
        w.writerow(["label", "5mb_1contig_wall_s", "5mb_1contig_cpu_s", "5mb_1contig_rss_mib", "5mb_200contigs_wall_s", "5mb_200contigs_rss_mib",
                    "panel_files", "panel_total_wall_s", "panel_total_cpu_s", "panel_max_rss_mib", "calls_equal_comparator"])
        for lab in labels:
            mine = [r for r in rows if r["label"] == lab]
            one = next(r for r in mine if r["input"].endswith("5mb_1contig.fna"))
            two = next(r for r in mine if r["input"].endswith("5mb_200contigs.fna"))
            panel = [r for r in mine if r["group"] == "panel"]
            eq = sum(calls[(lab, k[1])] == calls[("comparator", k[1])] for k in calls if k[0] == lab)
            n = sum(1 for k in calls if k[0] == lab)
            cpu = lambda r: (r["user_seconds"] or 0) + (r["sys_seconds"] or 0)
            w.writerow([lab, one["wall_seconds"], round(cpu(one), 2), round(one["max_rss_kib"] / 1024, 1), two["wall_seconds"], round(two["max_rss_kib"] / 1024, 1),
                        len(panel) or "", round(sum(r["wall_seconds"] for r in panel), 2) if panel else "", round(sum(cpu(r) for r in panel), 2) if panel else "",
                        round(max(r["max_rss_kib"] for r in panel) / 1024, 1) if panel else "", f"{eq}/{n}"])
    print("performance rows kept", len(rows), "; CPU seconds over all performance runs incl. repeated comparator passes:", round(total_cpu, 1))


if __name__ == "__main__":
    main(*sys.argv[1:3])
