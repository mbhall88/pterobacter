"""Shared helpers for the MLST feasibility harness (stdlib only)."""
import fcntl
import hashlib
import json
import os
import re
import signal
import subprocess
import sys
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
CPU_BUDGET_S = float(os.environ.get("MLST_CPU_BUDGET_S", 3600))
MAX_WORKERS = 4
LEDGER_ENV = "MLST_BUDGET_LEDGER"
HELPER = Path(__file__).with_name("_limit_exec.py")


class BudgetExceeded(RuntimeError):
    """The CPU budget cannot admit another run."""


class CpuBudget:
    """Run-level CPU budget with bounded admission.

    Before a run starts, `admit(reserve_s)` reserves its worst-case CPU (its CPU cap, set with RLIMIT_CPU, which equals the wall limit);
    it refuses when spent + reserved + reserve_s would exceed the limit. `settle` then replaces the reservation with the measured
    CPU. With `ledger` (a file path), the totals live in that file under an exclusive lock, so every driver process and thread
    sharing the ledger draws on one allowance; without it the budget is per process. A driver killed mid-run leaves its
    reservation in the ledger, which only makes later admission stricter. The bound is on the CPU of each run's own process (limited
    by RLIMIT_CPU) and holds for the single-threaded runs used here; a run that starts many busy child processes can use more.
    """

    def __init__(self, limit_s=CPU_BUDGET_S, ledger=None):
        self.limit_s, self.ledger, self._lock = limit_s, (Path(ledger) if ledger else None), threading.Lock()
        self._state = {"spent_s": 0.0, "reserved_s": 0.0}

    def _update(self, fn):
        with self._lock:
            if self.ledger is None:
                return fn(self._state)
            self.ledger.touch(exist_ok=True)
            with open(self.ledger, "r+") as fh:
                fcntl.flock(fh, fcntl.LOCK_EX)
                text = fh.read()
                state = json.loads(text) if text.strip() else {"spent_s": 0.0, "reserved_s": 0.0}
                result = fn(state)
                fh.seek(0)
                fh.truncate()
                fh.write(json.dumps(state))
                return result

    @property
    def spent_s(self):
        return self._update(lambda st: st["spent_s"])

    def admit(self, reserve_s):
        def go(st):
            if st["spent_s"] + st["reserved_s"] + reserve_s > self.limit_s:
                raise BudgetExceeded(f"CPU budget {self.limit_s}s cannot admit {reserve_s}s more (spent {st['spent_s']:.1f}s, reserved {st['reserved_s']:.1f}s)")
            st["reserved_s"] += reserve_s
        self._update(go)

    def settle(self, reserve_s, actual_s):
        def go(st):
            st["reserved_s"] = max(0.0, st["reserved_s"] - reserve_s)
            st["spent_s"] += actual_s
        self._update(go)


BUDGET = CpuBudget(ledger=os.environ.get(LEDGER_ENV))


def is_crash(rec):
    """A crash is a timeout or death by signal (GNU time reports 128+signal); ordinary non-zero exits are not crashes."""
    return bool(rec.get("timed_out")) or (rec.get("exit_code") or 0) >= 128


def timed(argv, *, stdout, stderr, env=None, cwd=None, timeout=None, mem_limit=None, budget=None):
    """Run argv under GNU time in its own process group, with a wall limit, CPU and address-space limits and the run-level CPU budget.

    Returns a provenance record (argv, exit, wall, user+sys CPU, peak RSS KiB, timed_out, limits). The run is admitted only if the
    budget can reserve `timeout` CPU seconds (raises BudgetExceeded otherwise, before anything starts). Limits are applied by
    `_limit_exec.py`, not preexec_fn, so launching from threads is safe. A run past the wall limit has its process group killed
    and is recorded with timed_out=True, exit_code=None, charged its wall time because its CPU is unknown.
    """
    timeout = WALL_LIMIT_S if timeout is None else timeout
    mem_limit = MEM_LIMIT_BYTES if mem_limit is None else mem_limit
    budget = BUDGET if budget is None else budget
    budget.admit(timeout)
    charged = 0.0
    try:
        time_file = Path(str(stdout) + ".time")
        full = [sys.executable, "-I", str(HELPER), str(mem_limit or 0), str(max(1, int(timeout))), "--",
                "/usr/bin/time", "-v", "-o", str(time_file), *map(str, argv)]
        started = time.monotonic()
        timed_out = False
        with open(stdout, "w") as out, open(stderr, "w") as err:
            proc = subprocess.Popen(full, stdout=out, stderr=err, env=env, cwd=cwd, start_new_session=True)
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
                   limits=dict(wall_seconds=timeout, cpu_seconds=max(1, int(timeout)), address_space_bytes=mem_limit),
                   stdout=str(stdout), stderr=str(stderr))
        charged = wall if timed_out else (rec["user_seconds"] or 0) + (rec["sys_seconds"] or 0)
        return rec
    finally:
        budget.settle(timeout, charged)


def cap_workers(n):
    if n < 1 or n > MAX_WORKERS:
        raise ValueError(f"--workers must be between 1 and {MAX_WORKERS} (protocol stop rule), got {n}")
    return n


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
