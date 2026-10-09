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

    def test_budget_admission_refuses_a_run_that_could_overspend(self):
        budget = common.CpuBudget(limit_s=5)
        with self.assertRaises(common.BudgetExceeded):
            self.run_py("print(1)", budget=budget, timeout=10)  # would reserve 10 CPU seconds
        self.assertEqual(budget.spent_s, 0)  # nothing started, nothing spent

    def test_budget_is_shared_across_driver_processes_through_the_ledger(self):
        import subprocess
        ledger = self.tmp / "ledger.json"
        script = ("import sys; sys.path.insert(0, %r); import common, pathlib\n"
                  "b = common.CpuBudget(limit_s=2.5, ledger=sys.argv[1])\n"
                  "t = pathlib.Path(sys.argv[2])\n"
                  "try:\n    common.timed([sys.executable, '-c', 'import time; time.sleep(30)'], stdout=t/'o', stderr=t/'e', timeout=1, budget=b)\n    print('ran')\n"
                  "except common.BudgetExceeded:\n    print('refused')\n") % str(HERE.parent)
        outs = [subprocess.run([sys.executable, "-c", script, str(ledger), str(self.tmp)], capture_output=True, text=True).stdout.strip() for _ in range(3)]
        # each run is killed at its 1 s limit and charged 1 s of a 2.5 s ledger; the third driver process starts fresh but sees the spend
        self.assertEqual(outs, ["ran", "ran", "refused"])
        state = json.loads(ledger.read_text())
        self.assertEqual(state["reserved_s"], 0)
        self.assertLessEqual(state["spent_s"], 2.5)

    def test_concurrent_runs_cannot_spend_the_same_allowance(self):
        import threading
        budget = common.CpuBudget(limit_s=1.0)
        barrier, results = threading.Barrier(6), []
        def job(i):
            barrier.wait()
            try:
                common.timed([sys.executable, "-c", "import time; time.sleep(30)"], stdout=self.tmp / f"o{i}", stderr=self.tmp / f"e{i}", timeout=0.5, budget=budget)
                results.append("admitted")
            except common.BudgetExceeded:
                results.append("refused")
        threads = [threading.Thread(target=job, args=(i,)) for i in range(6)]
        [t.start() for t in threads]; [t.join() for t in threads]
        self.assertEqual(results.count("admitted"), 2)  # 0.5 s reserved each against 1.0 s; settled spend keeps the rest out
        self.assertLessEqual(budget.spent_s, 1.0 + 0.3)

    def test_timeouts_hold_when_launched_from_threads(self):
        from concurrent.futures import ThreadPoolExecutor
        def job(i):
            return common.timed([sys.executable, "-c", "import time; time.sleep(30)"], stdout=self.tmp / f"t{i}", stderr=self.tmp / f"te{i}", timeout=1, budget=common.CpuBudget())
        with ThreadPoolExecutor(4) as ex:
            recs = list(ex.map(job, range(8)))
        self.assertTrue(all(r["timed_out"] for r in recs))
        self.assertLess(max(r["wall_seconds"] for r in recs), 6)

    def test_cpu_limit_kills_a_busy_loop(self):
        rec = self.run_py("while True: pass", timeout=1)
        self.assertTrue(common.is_crash(rec))

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

    def fixture_tree(self, names, fake_body):
        tmp = Path(tempfile.mkdtemp(prefix="cand-"))
        fixtures = tmp / "fixtures"
        (fixtures / "invalid").mkdir(parents=True)
        for n in names:
            (fixtures / f"{n}.fna").write_text(">c\nACGT\n")
        (fixtures / "manifest.json").write_text(json.dumps({"fixtures": {n: {"file": f"{n}.fna"} for n in names}}))
        fake = tmp / "fake-probe"
        fake.write_text(fake_body)
        fake.chmod(fake.stat().st_mode | stat.S_IEXEC)
        return tmp, fixtures, fake

    def test_rejects_more_than_four_workers(self):
        import subprocess
        tmp, fixtures, fake = self.fixture_tree(["fx0"], "#!/bin/sh\necho '{}'\n")
        r = subprocess.run([sys.executable, str(HERE.parent / "run_candidate.py"), str(fake), "x", str(tmp), str(fixtures), str(tmp / "out"), "--workers", "5"],
                           capture_output=True, text=True)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("--workers must be between 1 and 4", r.stderr)
        self.assertFalse((tmp / "out" / "fx0").exists(), "no job may start when the worker count is rejected")


class PerformanceStop(unittest.TestCase):
    """run_perf stops scheduling a candidate after its first crash, keeps the failure record and other candidates continue."""

    def test_crashed_candidate_is_not_rescheduled(self):
        import subprocess
        tmp = Path(tempfile.mkdtemp(prefix="perf-"))
        (tmp / "fix" / "perf" / "panel").mkdir(parents=True)
        for n in ("5mb_1contig", "5mb_200contigs"):
            (tmp / "fix" / "perf" / f"{n}.fna").write_text(">c\nACGT\n")
        for i in range(3):
            (tmp / "fix" / "perf" / "panel" / f"p{i}.fna").write_text(">c\nACGT\n")
        (tmp / "pkg" / "pubmlst" / "ecloacae").mkdir(parents=True)
        (tmp / "comp" / "blastdb").mkdir(parents=True)
        (tmp / "env" / "bin").mkdir(parents=True)
        mlst = tmp / "env" / "bin" / "mlst"
        mlst.write_text("#!/bin/sh\necho ok\n"); mlst.chmod(0o755)
        crashy, fine = tmp / "crashy", tmp / "fine"
        crashy.write_text("#!/bin/sh\nkill -SEGV $$\n"); fine.write_text("#!/bin/sh\necho ok\n")
        for f in (crashy, fine):
            f.chmod(0o755)
        bins = tmp / "bins.json"
        bins.write_text(json.dumps({"crashy": {"bin": str(crashy), "engine": "e"}, "fine": {"bin": str(fine), "engine": "e"}}))
        out = tmp / "perf.json"
        r = subprocess.run([sys.executable, str(HERE.parent / "run_perf.py"), str(tmp / "pkg"), str(tmp / "fix"), str(tmp / "comp"), str(bins), str(out)],
                           capture_output=True, text=True, env=dict(__import__("os").environ, MLST_ENV=str(tmp / "env")))
        self.assertEqual(r.returncode, 3, r.stderr)
        rows = json.loads(out.read_text())
        crashy_rows = [x for x in rows if x["label"] == "crashy"]
        self.assertEqual(len(crashy_rows), 1, "crashed candidate must not be scheduled again")
        self.assertGreaterEqual(crashy_rows[0]["exit_code"], 128)
        self.assertEqual(len([x for x in rows if x["label"] == "fine"]), 5)  # 2 single + 3 panel
        self.assertEqual(json.loads(out.with_suffix(".STOPPED.json").read_text()), {"crashy": "5mb_1contig.fna"})


if __name__ == "__main__":
    unittest.main()
