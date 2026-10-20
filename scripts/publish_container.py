"""Publish the tested image after its complete corresponding source is available."""
import argparse
import json
import os
from pathlib import Path
import subprocess

from check_release_tag import check
from publish_distribution import sha, upload_if_missing


def check_existing(tag, image_id):
    result = subprocess.run(['docker', 'manifest', 'inspect', tag], text=True, capture_output=True)
    if result.returncode:
        # Authentication, rate limits and network errors must not permit an overwrite.
        if 'no such manifest:' in result.stderr.lower() or 'manifest unknown' in result.stderr.lower():
            return False
        raise ValueError('Cannot establish whether the GHCR tag exists: ' + result.stderr)
    manifest = json.loads(result.stdout)
    if manifest.get('config', {}).get('digest') != image_id:
        raise ValueError('Existing GHCR tag differs; refusing replacement: ' + tag)
    return True


def publish(tag, assets, container, repository):
    identity = check(tag, publish=True)
    receipt = json.loads((container / 'container-receipt.json').read_text())
    version = identity['version']
    if receipt['source_commit'] != identity['source_commit'] or receipt['version'] != version:
        raise ValueError('Tested container has a different source or version')
    filename = f'pterobacter-{version}-linux-amd64-image.tar.gz'
    if receipt['archive'] != filename or receipt['architecture'] != 'linux/amd64':
        raise ValueError('Unexpected container archive or architecture')
    archive = container / filename
    if sha(archive) != receipt['archive_sha256']:
        raise ValueError('Container archive checksum mismatch')
    # Same source release as the native binaries; never publish an image before source.
    upload_if_missing(repository, tag, assets / f'pterobacter-{version}-corresponding-source.tar.gz')
    subprocess.run(['docker', 'image', 'load', '--input', str(archive)], check=True)
    image = json.loads(subprocess.check_output(['docker', 'image', 'inspect', receipt['image_id']], text=True))[0]
    labels = image['Config']['Labels']
    if (image['Id'] != receipt['image_id'] or image['Architecture'] != 'amd64' or image['Os'] != 'linux'
            or labels['org.opencontainers.image.revision'] != identity['source_commit']
            or labels['org.opencontainers.image.version'] != version):
        raise ValueError('Loaded image identity does not match the tested receipt')
    destination = f'ghcr.io/{repository.lower()}:{version}'
    if not check_existing(destination, image['Id']):
        subprocess.run(['docker', 'tag', image['Id'], destination], check=True)
        subprocess.run(['docker', 'push', destination], check=True)
    if not check_existing(destination, image['Id']):
        raise ValueError('Published image could not be verified')
    upload_if_missing(repository, tag, container / 'container-receipt.json')
    print('Verified container: ' + destination)


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--tag', required=True)
    p.add_argument('--assets', type=Path, required=True)
    p.add_argument('--container', type=Path, required=True)
    p.add_argument('--repository', required=True)
    p.add_argument('--approved-publication', action='store_true')
    a = p.parse_args()
    if (not a.approved_publication or os.environ.get('GITHUB_EVENT_NAME') != 'workflow_dispatch'
            or os.environ.get('GITHUB_REF') != 'refs/heads/main'):
        p.error('Use the approved release workflow from main')
    publish(a.tag, a.assets, a.container, a.repository)
