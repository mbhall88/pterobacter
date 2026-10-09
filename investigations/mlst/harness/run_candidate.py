#!/usr/bin/env python3
"""Run one probe binary/engine over the fixed fixtures.

    run_candidate.py BIN ENGINE PACKAGE_PUBMLST_DIR FIXTURE_DIR OUT_DIR [--preset P] [--workers N] [names...]
"""
import argparse
import json
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from common import sha256, timed


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("bin"); ap.add_argument("engine"); ap.add_argument("package"); ap.add_argument("fixtures"); ap.add_argument("out")
    ap.add_argument("--preset"); ap.add_argument("--workers", type=int, default=2); ap.add_argument("names", nargs="*")
    a = ap.parse_args()
    fixtures, out = Path(a.fixtures), Path(a.out)
    manifest = json.loads((fixtures / "manifest.json").read_text())
    jobs = [(n, fixtures / manifest["fixtures"][n]["file"]) for n in (a.names or manifest["fixtures"])]
    jobs += [("invalid_" + p.stem, p) for p in sorted((fixtures / "invalid").iterdir())]
    jobs.append(("invalid_missing_file", fixtures / "invalid" / "does_not_exist.fna"))

    def one(job):
        name, fa = job
        d = out / name
        d.mkdir(parents=True, exist_ok=True)
        argv = [a.bin, "--engine", a.engine, "--package", a.package, "--query", fa, "--hits-out", d / "hits.tsv", "--raw-out", d / "raw.tsv"]
        if a.preset:
            argv += ["--preset", a.preset]
        rec = timed(argv, stdout=d / "result.json", stderr=d / "stderr.txt")
        rec["fixture"] = name
        rec["input_sha256"] = sha256(fa) if fa.exists() else None
        (d / "record.json").write_text(json.dumps(rec, indent=1))
        return name, rec["exit_code"]

    with ThreadPoolExecutor(a.workers) as ex:
        for name, code in ex.map(one, jobs):
            if code:
                print(name, "exit", code)


if __name__ == "__main__":
    main()
