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
from measure import Runner
from summarize import summarize


class HarnessFailureTests(unittest.TestCase):
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
