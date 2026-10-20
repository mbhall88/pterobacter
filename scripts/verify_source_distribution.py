"""Rebuild corresponding source with an empty Cargo home and no network."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import tempfile
import tomllib


def verify(archive):
    with tempfile.TemporaryDirectory(prefix='ptero-source-check-', dir=os.environ.get('TMPDIR')) as temporary:
        root = Path(temporary)
        with tarfile.open(archive) as packed:
            seen = set()
            for member in packed:
                name = PurePosixPath(member.name)
                if (name.is_absolute() or '..' in name.parts or name.parts[0] != 'pterobacter-corresponding-source'
                        or not member.isfile() or member.name in seen):
                    raise ValueError('Unsafe corresponding source member')
                seen.add(member.name)
                target = root.joinpath(*name.parts)
                target.parent.mkdir(parents=True, exist_ok=True)
                with packed.extractfile(member) as source, target.open('xb') as sink:
                    while chunk := source.read(1024 * 1024):
                        sink.write(chunk)
                target.chmod(0o755 if member.mode & 0o111 else 0o644)
        source = root / 'pterobacter-corresponding-source'
        receipt = json.loads((source / 'SOURCE-ARCHIVE.json').read_text())
        expected = set(receipt['files']) | {'SOURCE-ARCHIVE.json'}
        if {str(p.relative_to(source)) for p in source.rglob('*') if p.is_file()} != expected:
            raise ValueError('Corresponding source file inventory mismatch')
        for name, item in receipt['files'].items():
            with (source / name).open('rb') as handle:
                if hashlib.file_digest(handle, 'sha256').hexdigest() != item['sha256']:
                    raise ValueError('Corresponding source hash mismatch: ' + name)
        env = {**os.environ, 'CARGO_HOME': str(root / 'empty-cargo-home'),
               'CARGO_TARGET_DIR': str(root / 'target'), 'CARGO_NET_OFFLINE': 'true'}
        install = root / 'install'
        subprocess.run(['cargo', 'install', '--path', str(source), '--locked', '--offline', '--root', str(install)], cwd=source, env=env, check=True)
        version = tomllib.loads((source / 'Cargo.toml').read_text())['package']['version']
        if sorted(p.name for p in (install / 'bin').iterdir()) != ['ptero', 'pterobacter']:
            raise ValueError('Unexpected source-build commands')
        for command in ['ptero', 'pterobacter']:
            got = subprocess.check_output([str(install / 'bin' / command), '--version'], text=True)
            if got != f'pterobacter {version}\n':
                raise ValueError('Corresponding source version mismatch')
        print(json.dumps({'source_commit': receipt['source_commit'], 'version': version,
                          'empty_cargo_home_offline_build': 'passed', 'commands': ['ptero', 'pterobacter']}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archive', type=Path)
    verify(parser.parse_args().archive)
