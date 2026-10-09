import contextlib
import csv
import io
import json
import pathlib
import sys
import tempfile
import time
import types
import unittest
from unittest import mock
import measure
from measure import Runner
from budget import CandidateStopped
from summarize import summarize


class HarnessFailureTests(unittest.TestCase):
    def args(self, root, workers=1):
        return types.SimpleNamespace(fixtures=root, workers=workers, threads=1,
            sample_pairs=False, pair_seconds=30, budget_ledger=root/'ledger.json',
            mummer_probe=pathlib.Path('/not-executed'), nucmer=pathlib.Path('/not-executed'),
            delta_filter=pathlib.Path('/not-executed'), minimap_probe=pathlib.Path('/not-executed'))

    def receipt(self, code):
        return dict(argv=['/not-executed'], exit_code=code, wall_seconds=.01,
                    cpu_seconds=0, peak_kib=0, sampled_tree_peak_kib=None)

    def fixtures(self, root):
        (root/'r.fa').write_text('>r\nACGT\n')
        (root/'q.fa').write_text('>q\nACGT\n')
        (root/'bad.fa').write_text('>bad\nACGTZ\n')
        valid = dict(id='first', reference='r.fa', query='q.fa', stratum='test', valid=True)
        invalid = dict(valid, id='invalid', reference='bad.fa', valid=False)
        return valid, invalid

    def test_signal_stops_reverse_later_matrix_invalid_and_panel(self):
        # Reproduce Astra's CLI-level injection without executing an aligner.
        with tempfile.TemporaryDirectory() as d:
            root = pathlib.Path(d)
            first, invalid = self.fixtures(root)
            cases = [first, dict(first, id='second'), invalid]
            manifest = dict(cases=cases, panel=[dict(first, id='panel')],
                sha256={name: measure.sha(root/name) for name in ('r.fa','q.fa','bad.fa')})
            (root/'manifest.json').write_text(json.dumps(manifest))
            argv = ['measure.py', '--fixtures', str(root), '--output', str(root/'outputs'),
                    '--budget-ledger', str(root/'ledger.json'), '--workers', '1', '--pair-seconds', '30', '--panel',
                    '--nucmer', '/not-executed', '--delta-filter', '/not-executed',
                    '--mummer-probe', '/not-executed', '--minimap-probe', '/not-executed',
                    '--candidates', 'embedded-mummer4']
            with mock.patch.object(sys, 'argv', argv), mock.patch.object(Runner, 'timed', return_value=self.receipt(139)) as timed, contextlib.redirect_stdout(io.StringIO()):
                measure.main()
            self.assertEqual(timed.call_count, 1)
            matrix = json.loads((root/'outputs/matrix.json').read_text())
            self.assertEqual(len(matrix), 1)
            self.assertEqual(matrix[0]['QR']['metrics']['status'], 'error')
            self.assertEqual(matrix[0]['QR']['receipts'][0]['exit_code'], 139)
            self.assertEqual(matrix[0]['RQ']['metrics']['status'], 'not_run')
            failure = json.loads((root/'outputs/matrix/embedded-mummer4/first/QR/failure.json').read_text())
            self.assertEqual(failure['failure_kind'], 'signal')
            self.assertEqual(failure['receipts'][0]['exit_code'], 139)
            self.assertFalse((root/'outputs/invalid').exists())
            self.assertEqual(json.loads((root/'outputs/panel.json').read_text())[0]['pairs'], 0)
            # A new Runner/invocation must not admit this candidate again.
            restarted = Runner(self.args(root))
            with mock.patch.object(restarted, 'timed') as native:
                result = restarted.pair('embedded-mummer4', first, root/'restart')
                self.assertEqual(native.call_count, 0)
            self.assertTrue(all(result[o]['metrics']['status'] == 'not_run' for o in ('QR','RQ')))
            data = json.loads((root/'ledger.json').read_text())
            self.assertEqual(data['entries'][0]['status'], 'failed')
            self.assertEqual(data['entries'][0]['charged_seconds'], 32)
            original = data['candidate_stops']['embedded-mummer4']
            restarted.stop_candidate('embedded-mummer4', dict(status='error', message='later failure'))
            self.assertEqual(json.loads((root/'ledger.json').read_text())['candidate_stops']['embedded-mummer4'], original)

    def test_invalid_signal_and_unknown_exits_are_errors_and_latched(self):
        # Even a validator-looking message cannot turn a signal into rejection.
        for code in (-11, 139, 134, 1, 2):
            with self.subTest(code=code), tempfile.TemporaryDirectory() as d:
                root = pathlib.Path(d)
                first, invalid = self.fixtures(root)
                runner = Runner(self.args(root))
                def crash(argv, directory, label, stdout):
                    (directory/'invalid.stderr').write_text('native failure' if code in (1,2) else 'invalid nucleotide alphabet')
                    return self.receipt(code)
                with mock.patch.object(runner, 'timed', side_effect=crash) as timed:
                    result = runner.invalid('embedded-mummer4', invalid, root/'invalid')
                    skipped = runner.pair('embedded-mummer4', first, root/'matrix')
                    self.assertEqual(timed.call_count, 1)
                self.assertEqual(result['status'], 'error')
                self.assertEqual(result['receipt']['exit_code'], code)
                self.assertEqual(skipped['QR']['metrics']['status'], 'not_run')
                with self.assertRaises(CandidateStopped):
                    runner.budget.reserve('next invocation', 30, 1, candidate='embedded-mummer4')

    def test_controlled_input_rejection_allows_later_cases(self):
        for candidate, code in (('embedded-mummer4',1), ('embedded-mummer3',2), ('asm5',2)):
            with self.subTest(candidate=candidate), tempfile.TemporaryDirectory() as d:
                root = pathlib.Path(d)
                _, alphabet = self.fixtures(root)
                (root/'duplicate.fa').write_text('>same\nA\n>same\nC\n')
                (root/'empty.fa').write_text('')
                invalids = [(alphabet, 'invalid nucleotide alphabet'),
                    (dict(alphabet,id='duplicate',reference='duplicate.fa'), 'duplicate or invalid FASTA ID'),
                    (dict(alphabet,id='empty',reference='empty.fa'), 'empty FASTA file or record'),
                    (dict(alphabet,id='missing',reference='missing.fa'), str(root/'missing.fa')+': No such file or directory (os error 2)')]
                runner = Runner(self.args(root))
                for case, message in invalids:
                    def reject(argv, directory, label, stdout):
                        (directory/'invalid.stderr').write_text(message)
                        return self.receipt(code)
                    with mock.patch.object(runner, 'timed', side_effect=reject):
                        result = runner.invalid(candidate, case, root/'invalid')
                    self.assertEqual(result['status'], 'invalid_input')
                    self.assertIsNone(runner.candidate_failure(candidate))
                runner.budget.reserve('later valid case', 30, 1, candidate=candidate).finish()

    def test_panel_stops_before_queued_siblings_or_later_batches(self):
        with tempfile.TemporaryDirectory() as d:
            root = pathlib.Path(d)
            first, _ = self.fixtures(root)
            runner = Runner(self.args(root, workers=2))
            # Order admission deterministically with one CPU slot, while the
            # executor queues both members of the first batch.
            second_slot = runner.slots.get()
            cases = [dict(first,id=str(i)) for i in range(5)]
            with mock.patch.object(runner, 'timed', return_value=self.receipt(139)) as timed:
                summary = runner.panel('embedded-mummer4', cases, root/'panel')
            runner.slots.put(second_slot)
            self.assertEqual(timed.call_count, 1)
            self.assertEqual(summary['pairs'], 2)
            self.assertEqual(summary['planned_pairs'], 5)

    def test_dummy_signal_preserves_gnu_time_failure_receipt(self):
        with tempfile.TemporaryDirectory() as d:
            root = pathlib.Path(d)
            runner = Runner(self.args(root))
            receipt = runner.standalone([sys.executable, '-c', 'import os,signal; os.kill(os.getpid(), signal.SIGTERM)'],
                                       root, 'signal', root/'stdout', 5, candidate='embedded-mummer4')
            self.assertEqual(receipt['exit_code'], 143)
            failure = json.loads((root/'signal.failure.json').read_text())
            self.assertEqual(failure['failure_kind'], 'signal')
            self.assertEqual(failure['receipt'], receipt)
            with mock.patch.object(measure.subprocess, 'Popen') as launch:
                with self.assertRaises(CandidateStopped):
                    runner.standalone([sys.executable, '-c', 'pass'], root, 'next', root/'next.stdout', 5, candidate='embedded-mummer4')
                launch.assert_not_called()

    def test_timeout_terminates_alignment_descendants(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            pidfile = root / 'descendant.pid'
            script = ('import pathlib,subprocess,sys,time; '
                      'p=subprocess.Popen([sys.executable,"-c","import time; time.sleep(30)"]); '
                      'pathlib.Path(sys.argv[1]).write_text(str(p.pid)); time.sleep(30)')
            runner = Runner(types.SimpleNamespace(sample_pairs=False))
            with self.assertRaisesRegex(RuntimeError, 'timeout'):
                runner.timed([sys.executable, '-c', script, str(pidfile)], root, 'timeout', root / 'stdout', timeout_seconds=.5)
            pid = int(pidfile.read_text())
            for _ in range(50):
                try:
                    state = pathlib.Path(f'/proc/{pid}/stat').read_text().split(') ')[1].split()[0]
                except FileNotFoundError:
                    break
                if state == 'Z':
                    break  # terminated, waiting for its OS parent to reap it
                time.sleep(.02)
            else:
                self.fail('alignment descendant still running after timeout')

    def test_failed_orientations_remain_in_published_summary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            first, fileapi, memory = [root / n for n in ['first', 'fileapi', 'memory']]
            def save_pair(folder, candidate, fixture, stratum):
                orientation = dict(metrics={'status': 'error'}, wall_seconds=.1,
                                   cpu_seconds=.01, peak_kib=0, sampled_tree_peak_kib=None)
                pair = dict(candidate=candidate, fixture=fixture, stratum=stratum,
                            QR=orientation, RQ=orientation)
                dest = folder / candidate / fixture
                dest.mkdir(parents=True)
                (dest / 'result.json').write_text(json.dumps(pair))
            for candidate in ['comparator', 'embedded-mummer3', 'asm5', 'asm10', 'asm20']:
                folder = first if candidate in ['comparator', 'embedded-mummer3'] else fileapi
                for i in range(20):
                    stratum = 'rescue-band' if i < 10 else 'AF-boundary'
                    save_pair(folder / 'matrix', candidate, str(i), stratum)
                    if candidate == 'embedded-mummer3':
                        save_pair(memory / 'matrix', candidate, str(i), stratum)
                save_pair(folder / 'panel', candidate, 'panel0', 'panel-cost')
                (folder / 'panel' / candidate / 'summary.json').write_text(json.dumps(dict(candidate=candidate, pairs=1)))
            output = root / 'published'
            with contextlib.redirect_stdout(io.StringIO()):
                summarize(first, fileapi, memory, output)
            summary = json.loads((output / 'summary.json').read_text())
            self.assertEqual(summary['matrix']['embedded-mummer3']['errors'], 40)
            self.assertEqual(summary['matrix']['embedded-mummer3']['exact_integer_pairs'], 0)
            self.assertEqual(summary['matrix']['embedded-mummer3']['normalized_delta_equal_pairs'], 0)
            with (output / 'observations.tsv').open() as stream:
                rows = list(csv.DictReader(stream, delimiter='\t'))
            self.assertEqual(len(rows), 200)
            self.assertTrue(all(r['status'] == 'error' and r['D'] == 'NA' and r['identity'] == 'NA' for r in rows))
            with (output / 'differences.tsv').open() as stream:
                diffs = list(csv.DictReader(stream, delimiter='\t'))
            self.assertTrue(all(r['identity_gate_changed'] == 'NA' for r in diffs))

if __name__ == '__main__':
    unittest.main()
