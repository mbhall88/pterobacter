"""Check notice preservation in the packaging export without running Cargo."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import yaml
import bundle_licenses as licenses


class DistributionNotices(unittest.TestCase):
    def setUp(self):
        self.bundle = json.loads(licenses.BUNDLE.read_text())
        self.supplement = licenses.SUPPLEMENT.read_text()
        self.export = licenses.distribution_bundle(self.bundle, self.supplement)

    def test_every_curated_cargo_text_survives_yaml_roundtrip(self):
        parsed = yaml.safe_load(licenses.distribution_yaml(self.bundle, self.supplement))
        self.assertEqual(parsed, self.export)
        self.assertEqual(parsed['third_party_libraries'], self.bundle['third_party_libraries'])
        self.assertFalse(any('NOT FOUND' in item['text']
                             for p in parsed['third_party_libraries'] for item in p['licenses']))
        names = {p['package_name'] for p in parsed['third_party_libraries']}
        self.assertTrue({'skani', 'partitions'} <= names)

    def test_supplement_is_preserved_except_obsolete_packaging_instruction(self):
        expected = self.supplement.replace('Ship this file with THIRDPARTY.json.',
                                          'The Cargo licence texts are included in this document.')
        self.assertEqual(self.export['supplemental_notices'], expected)
        self.assertIn('miniprot', expected)

    def test_native_documents_are_verbatim_with_source_hashes(self):
        documents = [n for n in self.export['native_notices'] if 'sha256' in n]
        for notice in documents:
            data = (licenses.ROOT / notice['source']).read_bytes()
            self.assertEqual(notice['text'], data.decode())
            self.assertEqual(notice['sha256'], hashlib.sha256(data).hexdigest())
        self.assertTrue({'LICENSE.md', 'GPL-3.0.txt', 'COPYRIGHT', 'ACKNOWLEDGEMENTS', 'README.md'}
                        <= {Path(n['source']).name for n in documents})

    def test_native_component_grants_are_retained(self):
        comments = [n for n in self.export['native_notices'] if 'line' in n]
        for component in ['jellyfish', 'mt_skip_list', 'compactsufsort', 'thread_pipe']:
            self.assertTrue(any('/' + component + '/' in n['source'] for n in comments), component)
        for notice in comments:
            source = (licenses.ROOT / notice['source']).read_text()
            self.assertIn(notice['text'], source)
            self.assertEqual(source.count('\n', 0, source.index(notice['text'])) + 1, notice['line'])

    def test_export_does_not_mutate_inputs_or_expose_workspace_paths(self):
        before = copy.deepcopy(self.bundle)
        encoded = licenses.distribution_yaml(self.bundle, self.supplement)
        self.assertEqual(self.bundle, before)
        self.assertNotIn(str(licenses.ROOT), encoded)
        self.assertIn('corresponding-source.tar.gz', encoded)
        self.assertIn('Downstream patches', encoded)

    def test_new_native_notices_are_discovered_without_inventory_edits(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            native = root / 'vendor/mummer4'
            native.mkdir(parents=True)
            for name in ['LICENSE.md', 'GPL-3.0.txt', 'COPYRIGHT', 'ACKNOWLEDGEMENTS', 'README.md']:
                (native / name).write_text('existing notice\n')
            (native / 'NOTICE.extra').write_text('new component notice\n')
            (native / 'new.hpp').write_text('#ifndef NEW\n/* Copyright Example.\nPermission is hereby granted. */\n')
            with patch.object(licenses, 'ROOT', root):
                notices = licenses.native_notices()
                self.assertTrue(any(n['text'] == 'new component notice\n' for n in notices))
                self.assertTrue(any('Copyright Example' in n['text'] for n in notices))
                (native / 'LICENSE.md').unlink()
                with self.assertRaises(FileNotFoundError):
                    licenses.native_notices()

    def test_missing_required_cargo_text_stops_generation(self):
        bundle = {'third_party_libraries': [{'package_name': 'missing', 'package_version': '1',
                  'license': 'MIT AND Apache-2.0', 'licenses': [
                      {'license': 'MIT', 'text': 'MIT text'},
                      {'license': 'Apache-2.0', 'text': 'NOT FOUND'}]}]}
        with patch.object(licenses, 'embedded_packages', return_value=[]):
            with self.assertRaisesRegex(ValueError, 'missing licence text'):
                licenses.complete_bundle(bundle)


if __name__ == '__main__':
    unittest.main()
