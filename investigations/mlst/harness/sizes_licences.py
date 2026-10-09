#!/usr/bin/env python3
"""Binary-size increments and dependency licences for each probe variant (run harness/build_variants.sh first)."""
import csv
import subprocess
import sys
from pathlib import Path

PROBE = Path(__file__).resolve().parents[1] / "probe"
OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else PROBE.parent / "results"
VARIANTS = [("core", None), ("exact-bio", "engine-exact-bio"), ("exact-ba", "engine-exact-blockaligner"),
            ("minimap2", "engine-minimap2"), ("minimap2-pure", "engine-minimap2-pure")]
OUT.mkdir(exist_ok=True)
empty = (PROBE / "target/core/release/empty").stat().st_size
core = (PROBE / "target/core/release/mlst-probe").stat().st_size
with (OUT / "binary-sizes.tsv").open("w") as fh:
    w = csv.writer(fh, delimiter="\t")
    w.writerow(["variant", "bytes", "increment_over_empty_executable", "increment_over_core_no_engine_build"])
    w.writerow(["empty", empty, 0, ""])
    for name, _ in VARIANTS:
        size = (PROBE / f"target/{name}/release/mlst-probe").stat().st_size
        w.writerow([name, size, size - empty, size - core])
seen = {}
for name, feat in VARIANTS:
    cmd = ["cargo", "tree", "-e", "normal", "--prefix", "none", "-f", "{p}|{l}"] + (["--features", feat] if feat else [])
    for line in subprocess.run(cmd, cwd=PROBE, capture_output=True, text=True, check=True).stdout.splitlines():
        pkg, lic = line.replace(" (*)", "").split("|")
        pkg = pkg.replace(" (proc-macro)", "")
        if pkg.startswith("mlst-feasibility"):
            continue
        seen.setdefault((pkg, lic), set()).add(name)
with (OUT / "dependency-licences.tsv").open("w") as fh:
    w = csv.writer(fh, delimiter="\t")
    w.writerow(["package", "licence", "variants"])
    for (pkg, lic), v in sorted(seen.items()):
        w.writerow([pkg, lic, ",".join(sorted(v))])
