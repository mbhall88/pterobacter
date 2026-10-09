"""Shared helpers for the MLST feasibility harness (stdlib only)."""
import hashlib
import json
import os
import re
import subprocess
import time
from pathlib import Path

LOCI = ["dnaA", "fusA", "gyrB", "leuS", "pyrG", "rplB", "rpoB"]
MLST_ENV = Path("/home/uqmhal11/sw/miniforge3/envs/mlst")


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def timed(argv, *, stdout, stderr, env=None, cwd=None):
    """Run argv under GNU time; return a provenance record (argv, exit, wall, user+sys CPU, peak RSS KiB)."""
    time_file = Path(str(stdout) + ".time")
    full = ["/usr/bin/time", "-v", "-o", str(time_file), *map(str, argv)]
    started = time.monotonic()
    with open(stdout, "w") as out, open(stderr, "w") as err:
        code = subprocess.run(full, stdout=out, stderr=err, env=env, cwd=cwd).returncode
    wall = time.monotonic() - started
    text = time_file.read_text() if time_file.exists() else ""
    def grab(pat):
        m = re.search(pat, text)
        return float(m.group(1)) if m else None
    rec = dict(argv=[str(a) for a in argv], exit_code=code, wall_seconds=round(wall, 4),
               user_seconds=grab(r"User time \(seconds\): ([\d.]+)"), sys_seconds=grab(r"System time \(seconds\): ([\d.]+)"),
               max_rss_kib=grab(r"Maximum resident set size \(kbytes\): (\d+)"),
               stdout=str(stdout), stderr=str(stderr))
    return rec


HIT = re.compile(r"^\[(\d+)\] (\S+):(\d+)-(\d+)\((plus|minus)\) \| (\w+) (\w+) (\d+) \| id=(\d+)/(\d+) \| cov=(\d+)/(\d+) \| seq=(\S+)$")


def parse_comparator(tsv_line, debug_text):
    """tsv_line: `--full` row. Returns tokens/ST/status/score and the raw hit list in BLAST output order."""
    cols = tsv_line.rstrip("\n").split("\t")
    label, scheme, st, status, score, alleles = cols[:6]
    tokens = {}
    for item in alleles.split(";") if alleles else []:
        m = re.fullmatch(r"(\w+)\((.*)\)", item)
        tokens[m.group(1)] = m.group(2)
    hits = []
    for line in debug_text.splitlines():
        m = HIT.match(line)
        if m:
            n, contig, s, e, strand, sch, gene, num, nident, alen, cov, hlen, seq = m.groups()
            hits.append(dict(order=int(n), contig=contig, start=min(int(s), int(e)), end=max(int(s), int(e)), strand=strand,
                             locus=gene, allele=num, nident=int(nident), alen=int(alen), hlen=int(hlen)))
    return dict(label=label, scheme=scheme, ST=st, status=status, score=int(score), tokens=tokens, hits=hits)
