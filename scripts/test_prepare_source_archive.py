"""Synthetic source-package safety tests; no dependency downloads or builds."""
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import prepare_source_archive as package


def packed(members):
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode='w') as archive:
        for name, kind, data, mode in members:
            info = tarfile.TarInfo(name)
            info.type, info.mode = kind, mode
            info.size = len(data) if kind == tarfile.REGTYPE else 0
            info.linkname = '../outside'
            archive.addfile(info, io.BytesIO(data) if info.isfile() else None)
    return output.getvalue()


class SourceArchiveTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def test_tracked_bytes_and_executable_modes_are_preserved(self):
        package.extract_tracked(packed([('scripts/build', tarfile.REGTYPE, b'hello', 0o755)]), self.root)
        self.assertEqual((self.root/'scripts/build').read_bytes(), b'hello')
        self.assertEqual(package.inventory(self.root)['scripts/build']['mode'], 0o755)

    def test_unsafe_members_are_refused(self):
        for name, kind in [('../outside', tarfile.REGTYPE), ('/outside', tarfile.REGTYPE),
                           ('link', tarfile.SYMTYPE), ('link', tarfile.LNKTYPE),
                           ('pipe', tarfile.FIFOTYPE)]:
            with self.subTest(name=name, kind=kind), self.assertRaises(ValueError):
                package.extract_tracked(packed([(name, kind, b'x', 0o644)]), self.root)

    def test_normalized_duplicate_is_refused(self):
        members = [('a', tarfile.REGTYPE, b'a', 0o644), ('./a', tarfile.REGTYPE, b'b', 0o644)]
        with self.assertRaises(ValueError):
            package.extract_tracked(packed(members), self.root)
        self.assertEqual((self.root/'a').read_bytes(), b'a')

    def test_inventory_rejects_links(self):
        (self.root/'link').symlink_to('missing')
        with self.assertRaises(ValueError):
            package.inventory(self.root)

    def test_archive_is_deterministic_and_never_replaces_output(self):
        source = self.root/'source'
        source.mkdir()
        (source/'a').write_bytes(b'example')
        first, second = self.root/'first.tar.gz', self.root/'second.tar.gz'
        package.write_archive(source, first)
        package.write_archive(source, second)
        self.assertEqual(first.read_bytes(), second.read_bytes())
        before = first.read_bytes()
        with self.assertRaises(FileExistsError):
            package.write_archive(source, first)
        self.assertEqual(first.read_bytes(), before)
        with tarfile.open(first) as archive:
            self.assertEqual(archive.getnames(), [package.ROOT_NAME+'/a'])
            self.assertEqual(archive.extractfile(archive.getmembers()[0]).read(), b'example')

    def test_refuses_output_in_checkout_and_dirty_source(self):
        source = self.root/'source'
        source.mkdir()
        with self.assertRaisesRegex(ValueError, 'outside'):
            package.prepare(source, source/'archive.tar.gz', self.root)
        with patch.object(package, 'run', return_value=' M modified'):
            with self.assertRaisesRegex(ValueError, 'clean checkout'):
                package.prepare(source, self.root/'archive.tar.gz', self.root)

    def test_output_symlink_is_not_followed(self):
        output = self.root/'archive.tar.gz'
        output.symlink_to('missing-target')
        with self.assertRaisesRegex(ValueError, 'symlink'):
            package.prepare(self.root/'source', output, self.root)
        self.assertFalse((self.root/'missing-target').exists())

    def test_vendoring_cannot_modify_tracked_source(self):
        original = packed([('Cargo.lock', tarfile.REGTYPE, b'locked', 0o644)])
        def fake_run(argv, cwd):
            if argv[0:2] == ['git', 'status']:
                return ''
            if argv[0:2] == ['git', 'rev-parse']:
                return 'a'*40
            (cwd/'Cargo.lock').write_bytes(b'changed')
            return '[source.vendored-sources]\ndirectory = "vendor-registry"\n'
        with patch.object(package, 'REQUIRED', ('Cargo.lock',)), \
                patch.object(package, 'run', side_effect=fake_run), \
                patch.object(package.subprocess, 'check_output', return_value=original):
            with self.assertRaisesRegex(ValueError, 'Vendoring changed'):
                package.prepare(self.root/'source', self.root/'archive.tar.gz', self.root)
        self.assertFalse((self.root/'archive.tar.gz').exists())

    def test_complete_manifest_binds_source_configuration_and_dependency(self):
        original = packed([('Cargo.lock', tarfile.REGTYPE, b'locked', 0o644)])
        def fake_run(argv, cwd):
            if argv[0:2] == ['git', 'status']:
                return ''
            if argv[0:2] == ['git', 'rev-parse']:
                return 'a'*40
            if argv[0:2] == ['cargo', '--version']:
                return 'cargo synthetic'
            dependency = cwd/'vendor-registry/example-1.0'
            dependency.mkdir(parents=True)
            (dependency/'.cargo-checksum.json').write_text('{}')
            (dependency/'lib.rs').write_bytes(b'pub fn example() {}')
            return '[source.vendored-sources]\ndirectory = "vendor-registry"\n'
        output = self.root/'archive.tar.gz'
        with patch.object(package, 'REQUIRED', ('Cargo.lock',)), \
                patch.object(package, 'run', side_effect=fake_run), \
                patch.object(package.subprocess, 'check_output', return_value=original):
            result = package.prepare(self.root/'source', output, self.root)
        self.assertEqual(result['registry_packages'], 1)
        with tarfile.open(output) as archive:
            manifest = json.load(archive.extractfile(package.ROOT_NAME+'/'+package.MANIFEST))
            actual = {member.name.split('/', 1)[1] for member in archive.getmembers()}
            self.assertEqual(actual, set(manifest['files']) | {package.MANIFEST})
            for name, item in manifest['files'].items():
                data = archive.extractfile(package.ROOT_NAME+'/'+name).read()
                self.assertEqual(package.hashlib.sha256(data).hexdigest(), item['sha256'])
        self.assertEqual(list(self.root.glob('pterobacter-source-*')), [])


if __name__ == '__main__':
    unittest.main()
