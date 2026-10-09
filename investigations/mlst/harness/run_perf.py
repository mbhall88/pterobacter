#!/usr/bin/env python3
"""Performance workloads: one 5 Mbp assembly (1 contig, 200 contigs) and a 20-assembly panel, run sequentially.

    run_perf.py PACKAGE_DIR FIXTURE_DIR COMPARATOR_DIR CANDIDATE_BINS_JSON OUT_JSON
CANDIDATE_BINS_JSON: {"label": {"bin": ..., "engine": ..., "extra": [...]}}
Setup/parse costs are included in the wall time of each process; fixture generation is not.
"""
import json
import os
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from common import MLST_ENV, BudgetExceeded, is_crash, timed


def main(package, fixtures, comp, bins, out):
    package, fixtures, comp = Path(package), Path(fixtures), Path(comp)
    bins = json.loads(Path(bins).read_text())
    pk = package / "pubmlst" / "ecloacae"
    work = Path(out).with_suffix(".work"); work.mkdir(exist_ok=True)
    env = dict(os.environ); env["PATH"] = f"{MLST_ENV / 'bin'}{os.pathsep}{env['PATH']}"
    files = [fixtures / "perf" / "5mb_1contig.fna", fixtures / "perf" / "5mb_200contigs.fna"]
    panel = sorted((fixtures / "perf" / "panel").glob("*.fna"))
    results = []
    stopped = {}  # label -> first crashed input; a crashed candidate is not scheduled again (protocol stop rule)
    def guarded(argv, d, e):
        try:
            return timed(argv, stdout=d / "stdout", stderr=d / "stderr", env=e)
        except BudgetExceeded as ex:
            stopped["*"] = str(ex)
            return dict(argv=[str(x) for x in argv], exit_code=None, not_run="cpu_budget_spent", detail=str(ex), wall_seconds=None, max_rss_kib=None)
    def run_one(label, fa, which):
        if label in stopped or stopped.get("*"):
            return
        d = work / label / fa.stem; d.mkdir(parents=True, exist_ok=True)
        if label == "comparator":
            with tempfile.TemporaryDirectory(prefix="mlstperf-", dir="/tmp") as tmp:
                argv = ["mlst", "--scheme", "ecloacae", "--datadir", package / "pubmlst", "--blastdb", comp / "blastdb" / "mlst.fa", "--minid", "95",
                        "--mincov", "50", "--threads", "1", "--full", "--nopath", fa]
                rec = guarded(argv, d, dict(env, TMPDIR=tmp))
        else:
            b = bins[label]
            argv = [b["bin"], "--engine", b["engine"], "--package", pk, "--query", fa, *b.get("extra", [])]
            rec = guarded(argv, d, None)
        rec.update(label=label, input=str(fa), group=which)
        results.append(rec)
        Path(out).write_text(json.dumps(results, indent=1))
        print(label, fa.name, rec["exit_code"], rec["wall_seconds"], rec["max_rss_kib"], flush=True)
        if is_crash(rec):
            stopped.setdefault(label, fa.name)
    for label in ["comparator", *bins]:
        for fa in files:
            run_one(label, fa, "single")
    for label in ["comparator", *[b for b in bins if not b.endswith(("asm5", "asm10", "only"))]]:
        for fa in panel:
            run_one(label, fa, "panel")
    if stopped:
        Path(out).with_suffix(".STOPPED.json").write_text(json.dumps(stopped, indent=1))
        print("STOPPED:", stopped, file=sys.stderr)
        sys.exit(3)


if __name__ == "__main__":
    main(*sys.argv[1:6])
