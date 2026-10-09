"""Shared helpers for the MLST feasibility harness (stdlib only)."""
import hashlib
import json
import os
import re
import resource
import signal
import subprocess
import threading
import time
from pathlib import Path

LOCI = ["dnaA", "fusA", "gyrB", "leuS", "pyrG", "rplB", "rpoB"]
MLST_ENV = Path(os.environ.get("MLST_ENV", "mlst-env-not-set"))  # prefix of the conda env holding mlst 2.32.2 + BLAST+ 2.16.0


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


# Protocol stop rules ("Equivalence and stopping"): 1 CPU-hour in total, 10 minutes wall per assembly, 4 workers, 2 GiB per worker.
WALL_LIMIT_S = 600
MEM_LIMIT_BYTES = 2 << 30
CPU_BUDGET_S = 3600
MAX_WORKERS = 4


class BudgetExceeded(RuntimeError):
    """The aggregate CPU budget is spent; no further subprocess is started."""


class CpuBudget:
    """Thread-safe running total of subprocess CPU seconds, charged after each run."""

    def __init__(self, limit_s=CPU_BUDGET_S):
        self.limit_s, self.spent_s, self._lock = limit_s, 0.0, threading.Lock()

    def check(self):
        with self._lock:
            if self.spent_s >= self.limit_s:
                raise BudgetExceeded(f"CPU budget {self.limit_s}s spent ({self.spent_s:.1f}s)")

    def charge(self, seconds):
        with self._lock:
            self.spent_s += seconds


BUDGET = CpuBudget()


def cap_workers(n):
    if n < 1 or n > MAX_WORKERS:
        raise ValueError(f"--workers must be between 1 and {MAX_WORKERS} (protocol stop rule), got {n}")
    return n


def is_crash(rec):
    """A crash is a timeout or death by signal (GNU time reports 128+signal); ordinary non-zero exits are not crashes."""
    return bool(rec.get("timed_out")) or (rec.get("exit_code") or 0) >= 128


def timed(argv, *, stdout, stderr, env=None, cwd=None, timeout=None, mem_limit=None, budget=None):
    """Run argv under GNU time in its own process group, with a wall limit, an address-space limit and the CPU budget.

    Returns a provenance record (argv, exit, wall, user+sys CPU, peak RSS KiB, timed_out, limits). A run that exceeds the
    wall limit has its whole process group killed and is recorded with timed_out=True, exit_code=None; it is charged its
    wall time as CPU because its real CPU is unknown. Raises BudgetExceeded before starting if the CPU budget is spent.
    """
    timeout = WALL_LIMIT_S if timeout is None else timeout
    mem_limit = MEM_LIMIT_BYTES if mem_limit is None else mem_limit
    budget = BUDGET if budget is None else budget
    budget.check()
    time_file = Path(str(stdout) + ".time")
    full = ["/usr/bin/time", "-v", "-o", str(time_file), *map(str, argv)]

    def limit():
        if mem_limit:
            resource.setrlimit(resource.RLIMIT_AS, (mem_limit, mem_limit))

    started = time.monotonic()
    timed_out = False
    with open(stdout, "w") as out, open(stderr, "w") as err:
        proc = subprocess.Popen(full, stdout=out, stderr=err, env=env, cwd=cwd, start_new_session=True, preexec_fn=limit)
        try:
            code = proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out, code = True, None
            os.killpg(proc.pid, signal.SIGKILL)
            proc.wait()
    wall = time.monotonic() - started
    text = time_file.read_text() if time_file.exists() else ""
    def grab(pat):
        m = re.search(pat, text)
        return float(m.group(1)) if m else None
    rec = dict(argv=[str(a) for a in argv], exit_code=code, timed_out=timed_out, wall_seconds=round(wall, 4),
               user_seconds=grab(r"User time \(seconds\): ([\d.]+)"), sys_seconds=grab(r"System time \(seconds\): ([\d.]+)"),
               max_rss_kib=grab(r"Maximum resident set size \(kbytes\): (\d+)"),
               limits=dict(wall_seconds=timeout, address_space_bytes=mem_limit),
               stdout=str(stdout), stderr=str(stderr))
    budget.charge(wall if timed_out else (rec["user_seconds"] or 0) + (rec["sys_seconds"] or 0))
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
