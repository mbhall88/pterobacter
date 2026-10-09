"""Tests for the harness stop controls, using tiny synthetic subprocesses only (no alignment work). Run: python3 -m unittest discover -s tests"""
import json
import stat
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import common  # noqa: E402


class TimedBounds(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="bounds-"))

    def run_py(self, code, **kw):
        return common.timed([sys.executable, "-c", code], stdout=self.tmp / "o", stderr=self.tmp / "e", budget=kw.pop("budget", common.CpuBudget()), **kw)

    def test_normal_run_is_recorded(self):
        rec = self.run_py("print(1)")
        self.assertEqual(rec["exit_code"], 0)
        self.assertFalse(rec["timed_out"])
        self.assertFalse(common.is_crash(rec))
        self.assertIsNotNone(rec["max_rss_kib"])

    def test_wall_limit_kills_process_group(self):
        marker = self.tmp / "child-survived"
        child = self.tmp / "child.py"
        child.write_text(f"import time, pathlib\ntime.sleep(3)\npathlib.Path({str(marker)!r}).write_text('x')\n")
        parent = self.tmp / "parent.py"
        parent.write_text(f"import subprocess, sys, time\nsubprocess.Popen([sys.executable, {str(child)!r}])\ntime.sleep(30)\n")
        code = f"exec(open({str(parent)!r}).read())"
        rec = self.run_py(code, timeout=1)
        self.assertTrue(rec["timed_out"])
        self.assertIsNone(rec["exit_code"])
        self.assertTrue(common.is_crash(rec))
        self.assertLess(rec["wall_seconds"], 10)
        import time
        time.sleep(4)
        self.assertFalse(marker.exists(), "grandchild outlived the timeout")

    def test_memory_limit_stops_runaway_allocation(self):
        rec = self.run_py("x = bytearray(600 * 1024 * 1024)", mem_limit=256 << 20)
        self.assertNotEqual(rec["exit_code"], 0)

    def test_segfault_counts_as_crash_and_plain_failure_does_not(self):
        self.assertTrue(common.is_crash(self.run_py("import os,signal; os.kill(os.getpid(), signal.SIGSEGV)")))
        self.assertFalse(common.is_crash(self.run_py("import sys; sys.exit(2)")))

    def test_cpu_budget_blocks_further_runs(self):
        budget = common.CpuBudget(limit_s=0.2)
        self.run_py("sum(range(30_000_000))", budget=budget)
        self.assertGreaterEqual(budget.spent_s, 0.2)
        with self.assertRaises(common.BudgetExceeded):
            self.run_py("print(1)", budget=budget)

    def test_worker_cap(self):
        self.assertEqual(common.cap_workers(4), 4)
        for bad in (0, 5):
            with self.assertRaises(ValueError):
                common.cap_workers(bad)


class CandidateStop(unittest.TestCase):
    """run_candidate stops after the first crash, keeps the failure record and writes STOPPED.json."""

    def test_stops_after_first_crash(self):
        import subprocess
        tmp = Path(tempfile.mkdtemp(prefix="cand-"))
        fixtures = tmp / "fixtures"
        (fixtures / "invalid").mkdir(parents=True)
        names = [f"fx{i}" for i in range(6)]
        for n in names:
            (fixtures / f"{n}.fna").write_text(">c\nACGT\n")
        (fixtures / "manifest.json").write_text(json.dumps({"fixtures": {n: {"file": f"{n}.fna"} for n in names}}))
        fake = tmp / "fake-probe"
        fake.write_text("#!/bin/sh\ncase \"$*\" in *fx2.fna*) kill -SEGV $$;; esac\necho '{}'\n")
        fake.chmod(fake.stat().st_mode | stat.S_IEXEC)
        out = tmp / "out"
        r = subprocess.run([sys.executable, str(HERE.parent / "run_candidate.py"), str(fake), "x", str(tmp), str(fixtures), str(out), "--workers", "1"],
                           capture_output=True, text=True)
        self.assertEqual(r.returncode, 3, r.stderr)
        stopped = json.loads((out / "STOPPED.json").read_text())
        self.assertEqual(stopped["crashed"], ["fx2"])
        self.assertEqual(set(stopped["not_run"]), {"fx3", "fx4", "fx5", "invalid_missing_file"})
        rec = json.loads((out / "fx2" / "record.json").read_text())
        self.assertGreaterEqual(rec["exit_code"], 128)
        self.assertFalse((out / "fx3" / "record.json").exists())

    def test_rejects_more_than_four_workers(self):
        import subprocess
        r = subprocess.run([sys.executable, str(HERE.parent / "run_candidate.py"), "b", "e", "p", str(HERE), str(self.tmp_out()), "--workers", "5"], capture_output=True, text=True)
        self.assertNotEqual(r.returncode, 0)

    def tmp_out(self):
        return Path(tempfile.mkdtemp(prefix="w5-"))


if __name__ == "__main__":
    unittest.main()
