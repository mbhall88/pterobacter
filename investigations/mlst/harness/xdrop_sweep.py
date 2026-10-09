#!/usr/bin/env python3
"""Re-run route D with other raw X-drop values and count HSP differences against the comparator (retains the calibration behind XDROP=13).

    xdrop_sweep.py PROBE_BIN SCHEME_DIR FIXTURE_DIR COMPARATOR_DIR WORK_DIR OUT_TSV [--values 10 11 ...]
SCHEME_DIR holds the allele FASTA and profile table (pubmlst/ecloacae). Each value runs the probe through run_candidate.py (so the
stop rules apply) and compare.py. Reported per value: fixtures at call parity, comparator HSPs, HSPs missing from / extra in route D.
Diagnostic only: the probe default (13) is what every reported result uses. Cost is about 0.3 s CPU per fixture per value.
"""
import argparse
import csv
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
DEFAULT_VALUES = [10, 11, 12, 13, 14, 15, 20, 30, 59]


def main():
    ap = argparse.ArgumentParser()
    for n in ("probe", "scheme", "fixtures", "comparator", "work", "out"):
        ap.add_argument(n)
    ap.add_argument("--values", type=int, nargs="*", default=DEFAULT_VALUES)
    a = ap.parse_args()
    work = Path(a.work)
    work.mkdir(parents=True, exist_ok=True)
    rows = []
    for v in a.values:
        cand = work / f"xdrop{v}"
        subprocess.run([sys.executable, "-I", str(HERE / "run_candidate.py"), a.probe, "ungapped", a.scheme, a.fixtures, str(cand), "--extra", f"--xdrop {v}"],
                       check=True, stdout=subprocess.DEVNULL)
        subprocess.run([sys.executable, "-I", str(HERE / "compare.py"), a.scheme, a.comparator, str(cand), str(work / f"cmp_xdrop{v}")],
                       check=True, stdout=subprocess.DEVNULL)
        res = [r for r in json.loads((work / f"cmp_xdrop{v}.json").read_text()) if r["kind"] == "valid"]
        rows.append(dict(xdrop=v, fixtures=len(res), call_parity_pass=sum(r["parity"] == "PASS" for r in res),
                         hsps_comparator=sum(r["hits_comparator"] for r in res), hsps_missing_in_route_d=sum(r["hits_missing_in_candidate"] for r in res),
                         hsps_extra_in_route_d=sum(r["hits_extra_in_candidate"] for r in res),
                         fixtures_with_hsp_difference=sum(bool(r["hits_missing_in_candidate"] or r["hits_extra_in_candidate"]) for r in res)))
        print(rows[-1], flush=True)
    with open(a.out, "w", newline="") as fh:
        w = csv.DictWriter(fh, list(rows[0]), delimiter="\t")
        w.writeheader()
        w.writerows(rows)


if __name__ == "__main__":
    main()
