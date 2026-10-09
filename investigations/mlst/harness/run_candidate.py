#!/usr/bin/env python3
"""Run one probe binary/engine over the fixed fixtures.

    run_candidate.py BIN ENGINE PACKAGE_PUBMLST_DIR FIXTURE_DIR OUT_DIR [--preset P] [--workers N] [names...]
"""
import argparse
import json
import sys
import threading
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from common import BudgetExceeded, cap_workers, is_crash, sha256, timed


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("bin"); ap.add_argument("engine"); ap.add_argument("package"); ap.add_argument("fixtures"); ap.add_argument("out")
    ap.add_argument("--preset"); ap.add_argument("--extra", default=""); ap.add_argument("--workers", type=int, default=2)
    ap.add_argument("--crash-limit", type=int, default=1, help="stop the candidate after this many crashes (signal death or timeout); protocol stop rule")
    ap.add_argument("--timeout", type=int, default=None, help="wall seconds per run (default: protocol limit)")
    ap.add_argument("names", nargs="*")
    a = ap.parse_args()
    fixtures, out = Path(a.fixtures), Path(a.out)
    manifest = json.loads((fixtures / "manifest.json").read_text())
    jobs = [(n, fixtures / manifest["fixtures"][n]["file"]) for n in (a.names or manifest["fixtures"])]
    jobs += [("invalid_" + p.stem, p) for p in sorted((fixtures / "invalid").iterdir())]
    jobs.append(("invalid_missing_file", fixtures / "invalid" / "does_not_exist.fna"))

    stop = threading.Event()
    crashes = []
    lock = threading.Lock()

    def one(job):
        name, fa = job
        if stop.is_set():
            return name, None
        d = out / name
        d.mkdir(parents=True, exist_ok=True)
        argv = [a.bin, "--engine", a.engine, "--package", a.package, "--query", fa, "--hits-out", d / "hits.tsv", "--raw-out", d / "raw.tsv"]
        if a.preset:
            argv += ["--preset", a.preset]
        argv += a.extra.split()
        try:
            rec = timed(argv, stdout=d / "result.json", stderr=d / "stderr.txt", timeout=a.timeout)
        except BudgetExceeded as e:
            stop.set()
            rec = dict(argv=[str(x) for x in argv], exit_code=None, not_run="cpu_budget_spent", detail=str(e))
        rec["fixture"] = name
        rec["input_sha256"] = sha256(fa) if fa.exists() else None
        (d / "record.json").write_text(json.dumps(rec, indent=1))
        if is_crash(rec):
            with lock:
                crashes.append(name)
                if len(crashes) >= a.crash_limit:
                    stop.set()
        return name, rec.get("exit_code")

    with ThreadPoolExecutor(cap_workers(a.workers)) as ex:
        for name, code in ex.map(one, jobs):
            if code is None and stop.is_set():
                continue
            if code:
                print(name, "exit", code)
    if stop.is_set():
        (out / "STOPPED.json").write_text(json.dumps(dict(reason="crash limit or CPU budget reached", crash_limit=a.crash_limit, crashed=crashes,
                                                          not_run=[n for n, _ in jobs if not (out / n / "record.json").exists()]), indent=1))
        print("STOPPED: crashed", crashes, "- see STOPPED.json", file=sys.stderr)
        sys.exit(3)


if __name__ == "__main__":
    main()
