import json
import pathlib
import tempfile
import unittest
from unittest import mock
import types
from measure import Runner
from budget import Budget, BudgetExhausted


class BudgetTests(unittest.TestCase):
    def test_shared_deadline_does_not_reset_between_stages_or_orientations(self):
        with tempfile.TemporaryDirectory() as d:
            clock = [100.0]
            lease = Budget(pathlib.Path(d) / 'ledger.json').reserve('pair', 30, 1, lambda: clock[0])
            self.assertEqual(lease.remaining(), 30)
            clock[0] += 12
            self.assertEqual(lease.remaining(), 18)
            clock[0] += 17
            self.assertEqual(lease.remaining(), 1)
            clock[0] += 1
            with self.assertRaisesRegex(RuntimeError, 'shared deadline'):
                lease.remaining()
            lease.finish(failed=True)

    def test_parallel_admission_and_cross_invocation_accounting(self):
        with tempfile.TemporaryDirectory() as d:
            p = pathlib.Path(d) / 'ledger.json'
            clock = [0.0]
            matrix = Budget(p, 100).reserve('matrix', 30, 1, lambda: clock[0])
            panel = Budget(p, 100).reserve('panel', 30, 2, lambda: clock[0])
            with self.assertRaises(BudgetExhausted):
                Budget(p, 100).reserve('diagnostic', 3, 1)
            clock[0] = 10
            matrix.finish()
            panel.finish()
            diagnostic = Budget(p, 100).reserve('diagnostic', 30, 2, lambda: clock[0])
            diagnostic.finish(failed=True)
            self.assertEqual(Budget.charged(json.loads(p.read_text())), 94)
            with self.assertRaises(BudgetExhausted):
                Budget(p, 100).reserve('next invocation', 5, 1)

    def test_crashed_invocations_keep_reservations_and_cannot_raise_limit(self):
        with tempfile.TemporaryDirectory() as d:
            p = pathlib.Path(d) / 'ledger.json'
            Budget(p, 32).reserve('unfinished', 30, 1)
            with self.assertRaises(BudgetExhausted):
                Budget(p, 32).reserve('restart', 1, 1)
            with self.assertRaisesRegex(ValueError, 'cannot change'):
                Budget(p, 33).reserve('larger', 1, 1)

    def test_runner_stops_reverse_orientation_after_shared_deadline(self):
        with tempfile.TemporaryDirectory() as d:
            root = pathlib.Path(d)
            runner = Runner(types.SimpleNamespace(fixtures=root, workers=1, sample_pairs=False, pair_seconds=30, budget_ledger=root/'ledger.json'))
            clock = [0.0]
            reserve = runner.budget.reserve
            runner.budget.reserve = lambda label, seconds, cpus: reserve(label, seconds, cpus, lambda: clock[0])
            def orientation(*args):
                clock[0] = 30
                return dict(metrics={'status':'no_alignment'}, wall_seconds=30, cpu_seconds=0, peak_kib=0, sampled_tree_peak_kib=None)
            with mock.patch.object(runner, 'orientation', side_effect=orientation) as align:
                r = runner.pair('dummy',dict(id='one', reference='r', query='q', stratum='test'),root/'matrix')
            self.assertEqual(align.call_count,1)
            self.assertEqual(r['QR']['metrics']['status'],'no_alignment')
            self.assertEqual(r['RQ']['metrics']['status'],'error')
            self.assertEqual(Budget.charged(json.loads((root/'ledger.json').read_text())),32)

    def test_dummy_stages_matrix_panel_and_diagnostic_share_ledger(self):
        import sys
        with tempfile.TemporaryDirectory() as d:
            root = pathlib.Path(d)
            args = types.SimpleNamespace(fixtures=root, workers=1, sample_pairs=False, pair_seconds=5, budget_ledger=root/'ledger.json')
            runner = Runner(args)
            def orientation(candidate, ref, query, directory):
                directory.mkdir(parents=True)
                receipts = [runner.timed([sys.executable,'-c','pass'], directory, label, directory/(label+'.stdout')) for label in ('align','filter')]
                return dict(metrics={'status':'no_alignment'},wall_seconds=sum(r['wall_seconds'] for r in receipts),cpu_seconds=sum(r['cpu_seconds'] for r in receipts),peak_kib=0,sampled_tree_peak_kib=None)
            case=dict(id='one',reference='r',query='q',stratum='test')
            with mock.patch.object(runner,'orientation',side_effect=orientation):
                runner.pair('dummy',case,root/'matrix')
                runner.panel('dummy',[case],root/'panel')
            restart=Runner(args)
            restart.timed([sys.executable,'-c','pass'],root,'diagnostic',root/'stdout')
            data=json.loads((root/'ledger.json').read_text())
            self.assertEqual(len(data['entries']),3)
            self.assertTrue(all(e['charged_seconds']>0 for e in data['entries']))
            self.assertTrue(all(e['status']=='complete' for e in data['entries']))

    def test_allocation_capacity_is_shared_across_invocations(self):
        with tempfile.TemporaryDirectory() as d:
            p=pathlib.Path(d)/'ledger.json'
            first=Budget(p)
            first.cpu_limit=2
            active=first.reserve('panel',30,2)
            second=Budget(p)
            second.cpu_limit=2
            with self.assertRaisesRegex(BudgetExhausted,'allocation'):
                second.reserve('index diagnostic',30,1)
            active.finish()
            second.reserve('after completion',30,1).finish()

    def test_failed_cleanup_overrun_is_charged_and_stops_future_work(self):
        with tempfile.TemporaryDirectory() as d:
            p=pathlib.Path(d)/'ledger.json'
            clock=[0.0]
            lease=Budget(p).reserve('timed out',30,2,lambda:clock[0])
            clock[0]=35
            lease.finish(failed=True)
            data=json.loads(p.read_text())
            self.assertEqual(data['entries'][0]['charged_seconds'],70)
            self.assertTrue(data['entries'][0]['cleanup_bound_exceeded'])
            with self.assertRaisesRegex(BudgetExhausted,'cleanup'):
                Budget(p).reserve('must stop',30,1)

if __name__ == '__main__':
    unittest.main()
