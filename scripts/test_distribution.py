"""Safety and source-preservation tests for distribution preparation."""
import io
import json
from pathlib import Path
import shutil
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import build_binary_distribution as dist
import check_embedded_sources as audit
import check_release_tag as release


class DistributionTests(unittest.TestCase):
    def test_embedded_sources_match_reviewed_implementation(self):
        audit.check()

    def test_source_audit_detects_an_algorithm_edit(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            shutil.copytree(audit.ROOT / 'vendor', root / 'vendor')
            (root / 'scripts').mkdir()
            shutil.copyfile(audit.ROOT / 'scripts/embedded-source-baseline.json', root / 'scripts/embedded-source-baseline.json')
            source = root / 'vendor/skani/src/chain.rs'
            source.write_text(source.read_text() + '\nfn unauthorized_calculation() {}\n')
            with patch.object(audit, 'ROOT', root), self.assertRaisesRegex(ValueError, 'Non-mechanical'):
                audit.check()

    def test_crate_extraction_rejects_links_traversal_and_duplicate_members(self):
        for names, link in [(['../escape'], False), (['/absolute'], False),
                            (['other/file'], False), (['pkg/link'], True),
                            (['pkg/file', 'pkg/file'], False),
                            (['pkg/scripts/__pycache__/x.pyc'], False),
                            (['pkg/target/cache'], False)]:
            with self.subTest(names=names, link=link), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary); archive = root / 'bad.crate'
                with tarfile.open(archive, 'w:gz') as packed:
                    for name in names:
                        info = tarfile.TarInfo(name)
                        if link:
                            info.type = tarfile.SYMTYPE; info.linkname = '/etc/passwd'
                            packed.addfile(info)
                        else:
                            info.size = 1; packed.addfile(info, io.BytesIO(b'x'))
                with self.assertRaises(ValueError):
                    dist.extract_crate(archive, root / 'out', 'pkg')

    def test_crate_extraction_retains_regular_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); archive = root / 'good.crate'
            with tarfile.open(archive, 'w:gz') as packed:
                info = tarfile.TarInfo('pkg/Cargo.toml'); info.size = 3
                packed.addfile(info, io.BytesIO(b'abc'))
            output = dist.extract_crate(archive, root / 'out', 'pkg')
            self.assertEqual((output / 'Cargo.toml').read_bytes(), b'abc')

    def test_macos_runtime_requirements_are_declared(self):
        dist.check_macos_runtime('ptero:\n /usr/lib/libSystem.B.dylib (version)\n'
                                 ' /opt/homebrew/opt/gcc/lib/libstdc++.6.dylib (version)\n'
                                 ' /opt/homebrew/opt/xz/lib/liblzma.5.dylib (version)')
        for path in ['/opt/homebrew/opt/undeclared/lib/new.dylib', '/tmp/build/libprivate.dylib']:
            with self.assertRaisesRegex(ValueError, 'Undocumented'):
                dist.check_macos_runtime('ptero:\n ' + path + ' (version)')

    def test_publication_requires_real_matching_tag(self):
        with patch.object(release, 'output', return_value='a' * 40):
            with self.assertRaisesRegex(ValueError, 'real reviewed version tag'):
                release.check('', publish=True)
            with self.assertRaisesRegex(ValueError, 'Expected a version tag'):
                release.check('--upload-all', publish=True)
            with self.assertRaisesRegex(ValueError, 'does not match'):
                release.check('v999.0.0', publish=True)


class SourceCITests(unittest.TestCase):
    commit = 'a' * 40

    def run_record(self, status='completed', conclusion='success', commit=None):
        return {'headSha': commit or self.commit, 'status': status, 'conclusion': conclusion}

    def test_already_successful_ci_needs_no_wait(self):
        with patch.object(release, 'output', return_value=json.dumps([self.run_record()])) as query, \
             patch.object(release.time, 'sleep') as sleep:
            release.wait_for_source_ci('owner/repo', self.commit)
            args = query.call_args.args[0]
            self.assertEqual(args[args.index('--event') + 1], 'push')
            self.assertEqual(args[args.index('--branch') + 1], 'main')
            self.assertEqual(args[args.index('--commit') + 1], self.commit)
            sleep.assert_not_called()

    def test_missing_then_running_ci_can_finish_successfully(self):
        responses = [[], [self.run_record('in_progress', '')], [self.run_record()]]
        with patch.object(release, 'output', side_effect=[json.dumps(x) for x in responses]), \
             patch.object(release.time, 'monotonic', side_effect=[0, 0, 15]), \
             patch.object(release.time, 'sleep') as sleep:
            release.wait_for_source_ci('owner/repo', self.commit, 60)
            self.assertEqual(sleep.call_count, 2)

    def test_terminal_failure_is_not_waited_out(self):
        for conclusion in ['failure', 'cancelled', 'timed_out', 'skipped', 'neutral']:
            with self.subTest(conclusion=conclusion), \
                 patch.object(release, 'output', return_value=json.dumps([self.run_record(conclusion=conclusion)])), \
                 patch.object(release.time, 'sleep') as sleep, \
                 self.assertRaisesRegex(ValueError, 'did not pass'):
                release.wait_for_source_ci('owner/repo', self.commit, 60)
            sleep.assert_not_called()

    def test_wrong_commit_success_cannot_authorize_publication(self):
        with patch.object(release, 'output', return_value=json.dumps([self.run_record(commit='b' * 40)])), \
             self.assertRaisesRegex(ValueError, 'No successful main CI'):
            release.wait_for_source_ci('owner/repo', self.commit)

    def test_wait_is_bounded(self):
        with patch.object(release, 'output', return_value='[]'), \
             patch.object(release.time, 'monotonic', side_effect=[0, 0, 5]), \
             patch.object(release.time, 'sleep') as sleep, \
             self.assertRaisesRegex(ValueError, 'wait expired'):
            release.wait_for_source_ci('owner/repo', self.commit, 5)
        sleep.assert_called_once_with(5)

    def test_api_failure_propagates_without_authorizing(self):
        with patch.object(release, 'output', side_effect=RuntimeError('API unavailable')), \
             self.assertRaisesRegex(RuntimeError, 'API unavailable'):
            release.wait_for_source_ci('owner/repo', self.commit, 60)


if __name__ == '__main__':
    unittest.main()
