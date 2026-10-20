"""Build and smoke-test a binary distribution from Cargo's actual package."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
NOTICES = ['LICENSE', 'LICENSE-GPL-3.0.txt', 'THIRDPARTY.json',
           'THIRD-PARTY-LICENSES.md', 'THIRD-PARTY-NOTICES.txt']
TARGETS = {'x86_64-unknown-linux-gnu': ('Linux', 'x86_64'),
           'aarch64-apple-darwin': ('Darwin', 'arm64')}


def sha(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def command(args, cwd=ROOT, env=None):
    print('+ ' + ' '.join(map(str, args)), flush=True)
    subprocess.run(list(map(str, args)), cwd=cwd, env=env, check=True)


def output(args, cwd=ROOT):
    return subprocess.check_output(list(map(str, args)), cwd=cwd, text=True).strip()


def extract_crate(path, destination, prefix):
    seen = set()
    with tarfile.open(path) as archive:
        for member in archive:
            name = PurePosixPath(member.name)
            if '__pycache__' in name.parts or 'target' in name.parts or name.suffix == '.pyc':
                raise ValueError('Build cache in Cargo archive: ' + member.name)
            if (name.is_absolute() or '..' in name.parts or name.parts[0] != prefix
                    or member.name in seen or not member.isfile()):
                raise ValueError('Unexpected Cargo archive member: ' + member.name)
            seen.add(member.name)
            target = destination.joinpath(*name.parts)
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.extractfile(member) as source, target.open('xb') as sink:
                shutil.copyfileobj(source, sink)
            target.chmod(0o755 if member.mode & 0o111 else 0o644)
    return destination / prefix


def write_archive(files, prefix, destination):
    with destination.open('xb') as raw, gzip.GzipFile(fileobj=raw, mode='wb', filename='', mtime=0) as zipped:
        with tarfile.open(fileobj=zipped, mode='w|') as archive:
            for name, path in sorted(files.items()):
                info = tarfile.TarInfo(prefix + '/' + name)
                info.size = path.stat().st_size
                info.mode = 0o755 if name.startswith('bin/') else 0o644
                with path.open('rb') as content:
                    archive.addfile(info, content)


def build(destination, target, allow_dirty=False):
    if target not in TARGETS or (platform.system(), platform.machine()) != TARGETS[target]:
        raise ValueError('Build on the native, supported target runner')
    dirty = bool(output(['git', 'status', '--porcelain', '--untracked-files=all']))
    if dirty and not allow_dirty:
        raise ValueError('Commit changes before preparing release artifacts')
    commit = output(['git', 'rev-parse', 'HEAD'])
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']['version']
    destination = destination.resolve()
    if destination == ROOT or ROOT in destination.parents:
        raise ValueError('Keep artifacts outside the source checkout')
    destination.mkdir(parents=True, exist_ok=False)
    command(['python3', 'scripts/check_embedded_sources.py'])
    command(['cargo', 'fetch', '--locked'])
    args = ['cargo', 'package', '--locked', '--offline', '--no-verify']
    if allow_dirty:
        args.append('--allow-dirty')
    command(args)
    metadata = json.loads(output(['cargo', 'metadata', '--locked', '--offline', '--no-deps', '--format-version', '1']))
    crate = Path(metadata['target_directory']) / 'package' / f'pterobacter-{version}.crate'
    copied = destination / crate.name
    shutil.copyfile(crate, copied)
    with tempfile.TemporaryDirectory(prefix='ptero-dist-', dir=os.environ.get('TMPDIR')) as tmp:
        tmp = Path(tmp)
        source = extract_crate(copied, tmp, f'pterobacter-{version}')
        manifest = tomllib.loads((source / 'Cargo.toml').read_text())
        if any(isinstance(d, dict) and ('path' in d or 'git' in d)
               for d in manifest.get('dependencies', {}).values()):
            raise ValueError('Packaged dependency is not a registry dependency')
        for notice in NOTICES:
            if not (source / notice).is_file():
                raise ValueError('Missing packaged notice: ' + notice)
        command(['python3', source / 'scripts/check_embedded_sources.py'], cwd=source)
        install = tmp / 'install'
        command(['cargo', 'install', '--path', source, '--locked', '--offline', '--root', install], cwd=source)
        binaries = install / 'bin'
        if sorted(p.name for p in binaries.iterdir()) != ['ptero', 'pterobacter']:
            raise ValueError('Unexpected installed commands')
        for name in ['ptero', 'pterobacter']:
            if output([binaries / name, '--version'], cwd=source) != f'pterobacter {version}':
                raise ValueError('Installed version mismatch')
        env = {**os.environ, 'TEROBACTER_INSTALLED_BIN': str(binaries)}
        command(['cargo', 'test', '--locked', '--offline', '--test', 'installed_smoke', '--', '--ignored', '--test-threads=1'], cwd=source, env=env)
        command(['cargo', 'test', '--locked', '--offline', '--test', 'skani_adapter', '--test', 'skani_seeding', '--test', 'skani_query_label', '--', '--test-threads=1'], cwd=source)
        inspect = ['ldd'] if platform.system() == 'Linux' else ['otool', '-L']
        runtime = {name: output([*inspect, binaries / name], cwd=source) for name in ['ptero', 'pterobacter']}
        runtime = {name: text.replace(str(binaries / name), name) for name, text in runtime.items()}
        files = {'bin/' + name: binaries / name for name in ['ptero', 'pterobacter']}
        files.update({name: source / name for name in NOTICES})
        instructions = tmp / 'README.txt'
        instructions.write_text(f'''Pterobacter {version} — {target}

Put bin/ptero and bin/pterobacter on PATH; they are equivalent commands.
Install the reference database: ptero database install 2026-10-15
Classify an assembly: ptero --report-dir isolate-report isolate.fa
Guide: https://mbhall88.github.io/pterobacter/

This native build requires the runtime libraries listed in manifest.json.
Linux builds use glibc and the GNU C++ runtime; they are not static musl builds.
Apple Silicon macOS builds require Homebrew GCC runtime libraries:
  brew install gcc
No Rust compiler or external analysis executable is needed to run these binaries.

Combined executable: GPL-3.0-or-later. Pterobacter-authored source: MIT.
Complete corresponding source: pterobacter-{version}-corresponding-source.tar.gz
provided beside this archive, from source commit {commit}.
See the accompanying component licences and notices.
''')
        files['README.txt'] = instructions
        receipt = {'format': 'pterobacter-binary-distribution-v1', 'version': version,
                   'target': target, 'source_commit': commit, 'dirty': dirty,
                   'rustc': output(['rustc', '-vV']), 'platform': platform.platform(),
                   'crate_sha256': sha(copied), 'runtime_libraries': runtime,
                   'files': {name: {'sha256': sha(p), 'bytes': p.stat().st_size} for name, p in files.items()}}
        receipt_path = tmp / 'manifest.json'
        receipt_path.write_text(json.dumps(receipt, indent=2, sort_keys=True) + '\n')
        files['manifest.json'] = receipt_path
        prefix = f'pterobacter-{version}-{target}'
        archive = destination / (prefix + '.tar.gz')
        write_archive(files, prefix, archive)
        # Verify the bytes actually distributed, including executable permissions.
        with tarfile.open(archive) as packed:
            got = {}
            for member in packed:
                name = str(PurePosixPath(member.name).relative_to(prefix))
                data = packed.extractfile(member).read()
                if hashlib.sha256(data).hexdigest() != sha(files[name]):
                    raise ValueError('Binary archive readback differs')
                if name.startswith('bin/') and member.mode != 0o755:
                    raise ValueError('Executable mode missing')
                got[name] = len(data)
            if set(got) != set(files):
                raise ValueError('Binary archive inventory differs')
        shutil.copyfile(receipt_path, destination / (prefix + '.json'))
    checksums = ''.join(f'{sha(p)}  {p.name}\n' for p in sorted(destination.iterdir()) if p.is_file())
    (destination / 'SHA256SUMS').write_text(checksums)
    print(json.dumps({'target': target, 'version': version, 'source_commit': commit,
                      'crate_sha256': sha(copied), 'archive_sha256': sha(archive)}, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--target', choices=TARGETS, required=True)
    parser.add_argument('--allow-dirty', action='store_true', help='local preparation only; forbidden for publication')
    args = parser.parse_args()
    build(args.output, args.target, args.allow_dirty)
