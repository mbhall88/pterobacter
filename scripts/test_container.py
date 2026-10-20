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


if __name__ == '__main__':
    unittest.main()
