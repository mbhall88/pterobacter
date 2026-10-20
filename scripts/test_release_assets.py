"""Check artifact identity and retry behavior without publishing anything."""
import hashlib
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError

import collect_release_assets as collector
import publish_distribution as publisher
import check_release_tag as release


def digest(data):
    return hashlib.sha256(data).hexdigest()


def tar(path, files):
    with tarfile.open(path, 'w:gz') as archive:
        for name, data in files.items():
            member = tarfile.TarInfo(name); member.size = len(data)
            member.mode = 0o755 if '/bin/' in name else 0o644
            archive.addfile(member, io.BytesIO(data))


def candidates(root):
    commit = 'a' * 40; version = '1.2.3'; crate = b'synthetic crate'
    (root / 'pterobacter-1.2.3.crate').write_bytes(crate)
    files = {name: b'synthetic bytes' for name in ['bin/ptero', 'bin/pterobacter', 'README.txt', 'LICENSE',
             'LICENSE-GPL-3.0.txt', 'THIRDPARTY.json', 'THIRD-PARTY-LICENSES.md', 'THIRD-PARTY-NOTICES.txt']}
    for target in collector.TARGETS:
        stem = f'pterobacter-{version}-{target}'
        receipt = {'source_commit': commit, 'version': version, 'target': target, 'dirty': False,
                   'crate_sha256': digest(crate),
                   'files': {name: {'sha256': digest(data), 'bytes': len(data)} for name, data in files.items()}}
        payload = json.dumps(receipt).encode()
        (root / (stem + '.json')).write_bytes(payload)
        tar(root / (stem + '.tar.gz'), {stem + '/' + name: data for name, data in {**files, 'manifest.json': payload}.items()})
    source_files = {'Cargo.toml': b'synthetic source'}
    manifest = {'source_commit': commit, 'files': {n: {'sha256': digest(d)} for n, d in source_files.items()}}
    source = root / 'pterobacter-1.2.3-corresponding-source.tar.gz'
    tar(source, {'pterobacter-corresponding-source/' + n: d for n, d in
                 {**source_files, 'SOURCE-ARCHIVE.json': json.dumps(manifest).encode()}.items()})
    (root / 'source-receipt.json').write_text(json.dumps({'source_commit': commit, 'archive_sha256': collector.sha(source)}))
    return version, commit


class ReleaseAssetsTests(unittest.TestCase):
    def test_matching_artifacts_are_collected_with_checksums(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); incoming = root / 'in'; incoming.mkdir()
            version, commit = candidates(incoming)
            collector.collect(incoming, root / 'out', version, commit)
            self.assertEqual(len((root / 'out' / 'SHA256SUMS').read_text().splitlines()), 7)

    def test_dirty_wrong_source_and_corrupt_crate_are_refused(self):
        for mutation in ['dirty', 'source_commit', 'crate']:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary); incoming = root / 'in'; incoming.mkdir()
                version, commit = candidates(incoming)
                if mutation == 'crate':
                    (incoming / 'pterobacter-1.2.3.crate').write_bytes(b'changed')
                else:
                    p = incoming / 'pterobacter-1.2.3-x86_64-unknown-linux-gnu.json'
                    d = json.loads(p.read_text()); d[mutation] = True if mutation == 'dirty' else 'b' * 40
                    p.write_text(json.dumps(d))
                with self.assertRaises(ValueError):
                    collector.collect(incoming, root / 'out', version, commit)
                self.assertFalse((root / 'out').exists())

    def test_inconsistent_platform_crates_are_refused(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); incoming = root / 'in'; incoming.mkdir()
            version, commit = candidates(incoming)
            other = incoming / 'other'; other.mkdir()
            (other / 'pterobacter-1.2.3.crate').write_bytes(b'different platform package')
            with self.assertRaisesRegex(ValueError, 'inconsistent'):
                collector.collect(incoming, root / 'out', version, commit)

    def test_registry_404_is_absent_but_server_failure_is_not(self):
        with patch.object(publisher, 'urlopen', side_effect=HTTPError('url', 404, 'missing', {}, None)):
            self.assertIsNone(publisher.registry_checksum('1.2.3'))
        with patch.object(publisher, 'urlopen', side_effect=HTTPError('url', 503, 'busy', {}, None)):
            with self.assertRaises(HTTPError):
                publisher.registry_checksum('1.2.3')

    def test_different_published_crate_cannot_be_overwritten(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); (root / 'pterobacter-1.2.3.crate').write_bytes(b'tested')
            with patch.object(publisher, 'check', return_value={'version': '1.2.3'}), \
                 patch.object(publisher, 'registry_checksum', return_value='different'), \
                 patch.object(publisher, 'upload_if_missing') as upload, \
                 patch.object(publisher.subprocess, 'run') as command:
                with self.assertRaisesRegex(ValueError, 'differs'):
                    publisher.publish('v1.2.3', root, 'owner/repo')
                upload.assert_not_called(); command.assert_not_called()

    def test_private_repository_is_refused_before_publication(self):
        with patch.object(release.tomllib, 'loads', return_value={'package': {'version': '1.2.3'}}), \
             patch.object(release, 'output', side_effect=['a' * 40, 'a' * 40, json.dumps({'visibility': 'PRIVATE'})]), \
             patch.object(release.subprocess, 'run'):
            with self.assertRaisesRegex(ValueError, 'private'):
                release.check('v1.2.3', publish=True)


if __name__ == '__main__':
    unittest.main()
