"""Compile and exercise the maintained native filter on synthetic tied alignments."""
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
import unittest


class NativeFilterTests(unittest.TestCase):
    def test_portable_stream_and_actual_filter(self):
        root = Path(__file__).resolve().parent.parent
        # Reuse the GNU toolchain required by the crate, including CI's Homebrew GCC.
        host = next(line[6:] for line in subprocess.check_output(
            ['rustc', '-vV'], text=True).splitlines() if line.startswith('host: '))
        keys = [f'CXX_{host}', 'CXX_' + host.replace('-', '_').replace('.', '_'),
                'HOST_CXX', 'CXX']
        key = next((key for key in keys if key in os.environ), None)
        if key is not None:
            compiler = shlex.split(os.environ[key])
            self.assertTrue(compiler, f'Empty compiler override: {key}')
        elif sys.platform == 'darwin':
            prefix = subprocess.check_output(['brew', '--prefix', 'gcc'], text=True).strip()
            candidates = list((Path(prefix) / 'bin').glob('g++-[0-9]*'))
            compiler = [str(max(candidates, key=lambda p: int(p.name.split('-')[-1])))]
        else:
            compiler = ['g++']
        fixture = root / 'tests/fixtures/native-filter'
        with tempfile.TemporaryDirectory() as directory:
            binary = str(Path(directory) / 'filter-check')
            command = compiler + ['-std=c++17', '-O3', '-I' + str(root / 'vendor/mummer4/include'),
                                  str(fixture / 'check.cc'),
                                  str(root / 'vendor/mummer4/src/tigr/delta.cc'),
                                  str(root / 'vendor/mummer4/src/tigr/tigrinc.cc'), '-o', binary]
            subprocess.run(command, check=True, timeout=120)
            subprocess.run([binary, str(fixture / 'tied.delta'), str(fixture / 'expected.filter')],
                           check=True, timeout=30)


if __name__ == '__main__':
    unittest.main()
