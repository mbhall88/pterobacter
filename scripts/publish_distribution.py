"""Publish already verified assets; retry only when existing bytes match exactly."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
from urllib.error import HTTPError
from urllib.request import Request, urlopen

from check_release_tag import check


def sha(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def registry_checksum(version):
    request = Request(f'https://crates.io/api/v1/crates/pterobacter/{version}',
                      headers={'User-Agent': 'Pterobacter release preparation; https://github.com/mbhall88/pterobacter'})
    try:
        with urlopen(request, timeout=30) as response:
            return json.load(response)['version']['checksum']
    except HTTPError as error:
        if error.code == 404:
            return None
        raise


def upload_if_missing(repository, tag, path):
    release = json.loads(subprocess.check_output(
        ['gh', 'api', f'repos/{repository}/releases/tags/{tag}'], text=True))
    existing = [a for a in release['assets'] if a['name'] == path.name]
    if len(existing) > 1:
        raise ValueError('Duplicate existing asset names')
    if existing:
        with tempfile.TemporaryDirectory(prefix='ptero-existing-asset-') as temporary:
            subprocess.run(['gh', 'release', 'download', tag, '--repo', repository,
                            '--pattern', path.name, '--dir', temporary], check=True)
            if sha(Path(temporary) / path.name) != sha(path):
                raise ValueError('Existing release asset differs; refusing replacement: ' + path.name)
        print('Existing asset matches: ' + path.name)
    else:
        subprocess.run(['gh', 'release', 'upload', tag, str(path), '--repo', repository], check=True)


def publish(tag, assets, repository):
    identity = check(tag, publish=True)
    version = identity['version']
    crate = assets / f'pterobacter-{version}.crate'
    # The workflow has compared this crate with the exact checkout's cargo package.
    expected = sha(crate)
    existing = registry_checksum(version)
    if existing is not None and existing != expected:
        raise ValueError('Existing crates.io version differs; never replace or silently skip it')
    # Source is accessible before any corresponding executable is distributed.
    upload_if_missing(repository, tag, assets / f'pterobacter-{version}-corresponding-source.tar.gz')
    if existing is None:
        subprocess.run(['cargo', 'publish', '--locked', '--no-verify'], check=True)
        if registry_checksum(version) != expected:
            raise ValueError('Published crate checksum has not been verified; retain artifacts and retry')
    else:
        print('Existing crates.io version matches the tested crate')
    for path in sorted(assets.iterdir()):
        if path.is_file() and not path.name.endswith('-corresponding-source.tar.gz'):
            upload_if_missing(repository, tag, path)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--assets', type=Path, required=True)
    parser.add_argument('--repository', required=True)
    parser.add_argument('--approved-publication', action='store_true')
    args = parser.parse_args()
    if not args.approved_publication or os.environ.get('GITHUB_EVENT_NAME') != 'workflow_dispatch':
        parser.error('Use the explicit approved publication workflow')
    publish(args.tag, args.assets, args.repository)
