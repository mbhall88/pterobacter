"""Verify binary/source identity and consolidate the assets for one release."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import tarfile

TARGETS = ['x86_64-unknown-linux-gnu', 'aarch64-apple-darwin']


def sha(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def collect(inputs, destination, version, commit):
    names = [f'pterobacter-{version}.crate', f'pterobacter-{version}-corresponding-source.tar.gz', 'source-receipt.json']
    for target in TARGETS:
        names += [f'pterobacter-{version}-{target}.tar.gz', f'pterobacter-{version}-{target}.json']
    selected = {}
    for name in names:
        matches = list(inputs.rglob(name))
        if not matches or len({sha(p) for p in matches}) != 1:
            raise ValueError('Missing or inconsistent artifact: ' + name)
        selected[name] = matches[0]
    crate_hash = sha(selected[f'pterobacter-{version}.crate'])
    for target in TARGETS:
        stem = f'pterobacter-{version}-{target}'
        receipt = json.loads(selected[stem + '.json'].read_text())
        if (receipt['source_commit'] != commit or receipt['version'] != version
                or receipt['target'] != target or receipt['dirty'] or receipt['crate_sha256'] != crate_hash):
            raise ValueError('Binary receipt does not match the approved release source')
        required = {'bin/ptero', 'bin/pterobacter', 'README.txt', 'LICENSE', 'LICENSE-GPL-3.0.txt', 'THIRDPARTY.json', 'THIRD-PARTY-LICENSES.md', 'THIRD-PARTY-NOTICES.txt'}
        if set(receipt['files']) != required:
            raise ValueError('Binary distribution lacks required commands or notices')
        expected = {stem + '/' + name for name in receipt['files']} | {stem + '/manifest.json'}
        with tarfile.open(selected[stem + '.tar.gz']) as archive:
            if len(archive.getmembers()) != len(expected) or set(archive.getnames()) != expected:
                raise ValueError('Unexpected binary archive inventory')
            for member in archive:
                if not member.isfile():
                    raise ValueError('Non-regular binary archive member')
                name = member.name.removeprefix(stem + '/')
                data = archive.extractfile(member).read()
                if name == 'manifest.json':
                    if json.loads(data) != receipt:
                        raise ValueError('Embedded and outer receipt differ')
                elif (hashlib.sha256(data).hexdigest() != receipt['files'][name]['sha256']
                      or len(data) != receipt['files'][name]['bytes']):
                    raise ValueError('Binary archive hash mismatch')
                if name.startswith('bin/') and member.mode != 0o755:
                    raise ValueError('Executable permissions missing')
    source = selected[f'pterobacter-{version}-corresponding-source.tar.gz']
    source_receipt = json.loads(selected['source-receipt.json'].read_text())
    if source_receipt['source_commit'] != commit or source_receipt['archive_sha256'] != sha(source):
        raise ValueError('Corresponding source receipt mismatch')
    with tarfile.open(source) as archive:
        manifest = json.load(archive.extractfile('pterobacter-corresponding-source/SOURCE-ARCHIVE.json'))
        if manifest['source_commit'] != commit:
            raise ValueError('Corresponding source commit mismatch')
        # Check the complete source payload; this is the archive users will receive.
        expected = {'pterobacter-corresponding-source/' + name for name in manifest['files']}
        expected.add('pterobacter-corresponding-source/SOURCE-ARCHIVE.json')
        if len(archive.getmembers()) != len(expected) or set(archive.getnames()) != expected:
            raise ValueError('Corresponding source inventory mismatch')
        for member in archive:
            if not member.isfile():
                raise ValueError('Non-regular corresponding source member')
            name = member.name.removeprefix('pterobacter-corresponding-source/')
            if name == 'SOURCE-ARCHIVE.json':
                continue
            with archive.extractfile(member) as data:
                if hashlib.file_digest(data, 'sha256').hexdigest() != manifest['files'][name]['sha256']:
                    raise ValueError('Corresponding source hash mismatch')
    destination.mkdir(parents=True, exist_ok=False)
    for name, path in selected.items():
        shutil.copyfile(path, destination / name)
    (destination / 'SHA256SUMS').write_text(''.join(f'{sha(destination / name)}  {name}\n' for name in sorted(selected)))
    print(f'Verified {len(selected)} assets for {version} at {commit}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--commit', required=True)
    args = parser.parse_args()
    collect(args.input, args.output, args.version, args.commit)
