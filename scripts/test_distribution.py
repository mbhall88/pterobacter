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

    def test_publication_requires_real_matching_tag(self):
        with patch.object(release, 'output', return_value='a' * 40):
            with self.assertRaisesRegex(ValueError, 'real reviewed version tag'):
                release.check('', publish=True)
            with self.assertRaisesRegex(ValueError, 'Expected a version tag'):
                release.check('--upload-all', publish=True)
            with self.assertRaisesRegex(ValueError, 'does not match'):
                release.check('v999.0.0', publish=True)


if __name__ == '__main__':
    unittest.main()
