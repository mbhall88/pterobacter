"""Publication must refuse conflicting tags and ambiguous registry failures."""
import json
import subprocess
import unittest
from unittest.mock import patch

import publish_container as publish


class ContainerPublicationTests(unittest.TestCase):
    def remote(self, code=0, stdout='', stderr=''):
        return patch.object(publish.subprocess, 'run', return_value=subprocess.CompletedProcess([], code, stdout, stderr))

    def test_matching_image_is_a_safe_retry(self):
        with self.remote(stdout=json.dumps({'config': {'digest': 'sha256:abc'}})):
            self.assertTrue(publish.check_existing('ghcr.io/example/image:1', 'sha256:abc'))

    def test_different_image_is_never_overwritten(self):
        with self.remote(stdout=json.dumps({'config': {'digest': 'sha256:other'}})):
            with self.assertRaisesRegex(ValueError, 'refusing replacement'):
                publish.check_existing('ghcr.io/example/image:1', 'sha256:abc')

    def test_multi_platform_manifest_is_not_silently_replaced(self):
        with self.remote(stdout=json.dumps({'manifests': []})):
            with self.assertRaisesRegex(ValueError, 'refusing replacement'):
                publish.check_existing('ghcr.io/example/image:1', 'sha256:abc')

    def test_missing_manifest_allows_first_publication(self):
        for message in ('no such manifest: ghcr.io/example/image:1', 'manifest unknown'):
            with self.subTest(message=message), self.remote(1, stderr=message):
                self.assertFalse(publish.check_existing('ghcr.io/example/image:1', 'sha256:abc'))

    def test_authentication_network_and_rate_limits_fail_closed(self):
        for message in ('unauthorized', 'connection reset', '429 too many requests', ''):
            with self.subTest(message=message), self.remote(1, stderr=message):
                with self.assertRaisesRegex(ValueError, 'Cannot establish'):
                    publish.check_existing('ghcr.io/example/image:1', 'sha256:abc')

class ContainerReceiptTests(unittest.TestCase):
    def setUp(self):
        import tempfile
        from pathlib import Path
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.archive = self.root / 'pterobacter-1.0.0-linux-amd64-image.tar.gz'
        self.archive.write_bytes(b'candidate image')
        self.receipt = {'version': '1.0.0', 'source_commit': 'a' * 40,
                        'architecture': 'linux/amd64', 'archive': self.archive.name,
                        'image_id': 'sha256:abc', 'archive_sha256': publish.sha(self.archive)}
        self.identity = {'version': '1.0.0', 'source_commit': 'a' * 40}

    def attempt(self):
        (self.root / 'container-receipt.json').write_text(json.dumps(self.receipt))
        return publish.publish('1.0.0', self.root, self.root, 'example/image')

    def test_unreviewed_identity_or_archive_never_reaches_source_upload(self):
        cases = [('source_commit', 'b' * 40), ('version', '2.0.0'),
                 ('archive', '../foreign.tar.gz'), ('architecture', 'linux/arm64'),
                 ('archive_sha256', '0' * 64)]
        for field, value in cases:
            with self.subTest(field=field):
                original = self.receipt[field]
                self.receipt[field] = value
                with patch.object(publish, 'check', return_value=self.identity), \
                     patch.object(publish, 'upload_if_missing') as upload:
                    with self.assertRaises(ValueError):
                        self.attempt()
                    upload.assert_not_called()
                self.receipt[field] = original

    def test_source_is_available_before_any_image_push(self):
        events = []
        image = {'Id': 'sha256:abc', 'Architecture': 'amd64', 'Os': 'linux',
                 'Config': {'Labels': {'org.opencontainers.image.revision': 'a' * 40,
                                       'org.opencontainers.image.version': '1.0.0'}}}
        def upload(repo, tag, path):
            events.append('source' if path.name.endswith('corresponding-source.tar.gz') else 'receipt')
        def run(args, **kwargs):
            events.append(args[1])
        with patch.object(publish, 'check', return_value=self.identity), \
             patch.object(publish, 'upload_if_missing', side_effect=upload), \
             patch.object(publish.subprocess, 'run', side_effect=run), \
             patch.object(publish.subprocess, 'check_output', return_value=json.dumps([image])), \
             patch.object(publish, 'check_existing', side_effect=[False, True]):
            self.attempt()
        self.assertEqual(events, ['source', 'image', 'tag', 'push', 'receipt'])


if __name__ == '__main__':
    unittest.main()
