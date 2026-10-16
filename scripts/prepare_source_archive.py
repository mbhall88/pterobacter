"""Prepare corresponding source from a clean commit; never publish or modify Git.

Requires Python 3.9+, Git, Cargo and cached locked dependency sources. Temporary
vendoring stays under TMPDIR or --staging-parent and is removed on exit.
"""
import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import tempfile


ROOT_NAME = 'pterobacter-corresponding-source'
MANIFEST = 'SOURCE-ARCHIVE.json'
REQUIRED = (
    'Cargo.toml', 'Cargo.lock', 'build.rs', 'native/anim.cc',
    'LICENSE', 'LICENSE-GPL-3.0.txt', 'THIRD-PARTY-LICENSES.md',
    'THIRDPARTY.json', 'THIRD-PARTY-NOTICES.txt',
    'vendor/mummer4/LICENSE.md', 'vendor/mummer4/GPL-3.0.txt',
    'vendor/mummer4/COPYRIGHT', 'vendor/mummer4/ACKNOWLEDGEMENTS',
    'vendor/mummer4/README.md', 'vendor/mummer4/upstream-files.json',
    'investigations/anim/mummer4-maintenance.patch',
    'investigations/anim/filter-portability.patch',
    'vendor/mummer4/include/mummer/pterobacter_filter_rng.hh',
    'vendor/skani/LICENSE', 'vendor/partitions/LICENSE.txt',
    'docs/licensing.md',
)


def digest(path):
    state = hashlib.sha256()
    with path.open('rb') as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b''):
            state.update(chunk)
    return state.hexdigest()


def run(argv, cwd):
    return subprocess.check_output(argv, cwd=cwd, text=True)


def extract_tracked(export, destination):
    """Reject links and ambiguous paths even though Git generated this archive."""
    seen = set()
    with tarfile.open(fileobj=io.BytesIO(export)) as archive:
        for member in archive:
            path = PurePosixPath(member.name)
            name = str(path)
            if path.is_absolute() or '..' in path.parts or name in seen:
                raise ValueError('Unsafe or duplicate source path')
            seen.add(name)
            if member.isdir():
                continue
            if not member.isfile():
                raise ValueError('Source archive contains a link or special file')
            target = destination / name
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.extractfile(member) as source, target.open('xb') as output:
                while chunk := source.read(1024 * 1024):
                    output.write(chunk)
            target.chmod(0o755 if member.mode & 0o111 else 0o644)


def inventory(directory):
    result = {}
    for path in sorted(directory.rglob('*')):
        if path.is_symlink():
            raise ValueError('Symlinks are not supported in the source package')
        if path.is_file():
            name = str(path.relative_to(directory))
            result[name] = {'sha256': digest(path), 'bytes': path.stat().st_size,
                            'mode': 0o755 if path.stat().st_mode & 0o111 else 0o644}
        elif not path.is_dir():
            raise ValueError('Special file in source package')
    return result


def write_archive(directory, output):
    """Atomic no-replacement publication of the local archive, not a release."""
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=output.parent, suffix='.partial') as temporary:
        with gzip.GzipFile(fileobj=temporary, mode='wb', filename='', mtime=0) as zipped:
            with tarfile.open(fileobj=zipped, mode='w|') as archive:
                for name, item in inventory(directory).items():
                    info = tarfile.TarInfo(ROOT_NAME + '/' + name)
                    info.size = item['bytes']
                    info.mode = item['mode']
                    with (directory / name).open('rb') as handle:
                        archive.addfile(info, handle)
        temporary.flush()
        os.link(temporary.name, output)


def prepare(source, output, staging_parent):
    if output.is_symlink():
        raise ValueError('Output must not be a symlink')
    source, output = source.resolve(), output.resolve()
    if output.exists():
        raise ValueError('Output already exists')
    if source == output or source in output.parents:
        raise ValueError('Keep generated archives outside the source checkout')
    if run(['git', 'status', '--porcelain', '--untracked-files=all'], source).strip():
        raise ValueError('Commit changes and use a clean checkout before packaging')
    commit = run(['git', 'rev-parse', 'HEAD'], source).strip()
    export = subprocess.check_output(['git', 'archive', '--format=tar', commit], cwd=source)
    with tempfile.TemporaryDirectory(prefix='pterobacter-source-', dir=staging_parent) as temporary:
        stage = Path(temporary) / ROOT_NAME
        stage.mkdir()
        extract_tracked(export, stage)
        tracked = inventory(stage)
        if any(name not in tracked for name in REQUIRED):
            raise ValueError('Required source, notice or maintenance patch missing')
        if MANIFEST in tracked or (stage / 'vendor-registry').exists():
            raise ValueError('Generated source-package paths already present')
        config_path = stage / '.cargo/config.toml'
        if config_path.exists() or (stage / '.cargo/config').exists():
            raise ValueError('Refuse to replace existing Cargo source configuration')
        command = ['cargo', 'vendor', '--locked', '--offline', '--versioned-dirs', 'vendor-registry']
        config = run(command, stage)
        if 'directory = "vendor-registry"' not in config:
            raise ValueError('Cargo did not emit the expected relative source directory')
        config_path.parent.mkdir(exist_ok=True)
        config_path.write_text(config)
        files = inventory(stage)
        if any(files.get(name) != item for name, item in tracked.items()):
            raise ValueError('Vendoring changed tracked source bytes or modes')
        checksums = sorted((stage / 'vendor-registry').glob('*/.cargo-checksum.json'))
        if not checksums:
            raise ValueError('No locked registry dependency sources were packaged')
        manifest = {
            'format': 'pterobacter-corresponding-source-v1', 'source_commit': commit,
            'cargo_lock_sha256': tracked['Cargo.lock']['sha256'],
            'cargo_version': run(['cargo', '--version'], stage).strip(),
            'vendor_command': command, 'registry_packages': len(checksums),
            'tracked_files': sorted(tracked), 'files': files,
            'scope': 'Source and notices; excludes compilers and system libraries. No database or evaluation payload.',
        }
        (stage / MANIFEST).write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
        write_archive(stage, output)
    return {'source_commit': commit, 'archive_sha256': digest(output),
            'archive_bytes': output.stat().st_size, 'files': len(files) + 1,
            'registry_packages': len(checksums),
            'manifest_sha256': hashlib.sha256((json.dumps(manifest, indent=2, sort_keys=True) + '\n').encode()).hexdigest()}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--staging-parent', type=Path, default=os.environ.get('TMPDIR'))
    args = parser.parse_args()
    if args.staging_parent is None:
        parser.error('Set TMPDIR or provide --staging-parent for temporary vendoring')
    print(json.dumps(prepare(args.source, args.output, args.staging_parent), indent=2))
