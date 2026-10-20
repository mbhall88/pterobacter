"""Exercise the actual runtime image with existing synthetic database fixtures."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import subprocess


def command(args):
    return subprocess.check_output(args, text=True).strip()


def check(image, fixture, output, version, commit):
    output.mkdir()
    # Bind mounts must be writable by the image's unprivileged user.
    for name in ('database', 'work'):
        (output / name).mkdir(mode=0o777)
        (output / name).chmod(0o777)
    inspect = json.loads(command(['docker', 'image', 'inspect', image]))[0]
    assert inspect['Architecture'] == 'amd64' and inspect['Os'] == 'linux'
    assert inspect['Config']['User'] == '10001:10001'
    assert inspect['Config']['Entrypoint'] == ['ptero']
    assert inspect['Size'] < 200_000_000, 'Runtime exceeds the 200 MB uncompressed budget'
    labels = inspect['Config']['Labels']
    assert labels['org.opencontainers.image.revision'] == commit
    assert labels['org.opencontainers.image.version'] == version
    assert labels['org.opencontainers.image.licenses'] == 'GPL-3.0-or-later'
    base = ['docker', 'run', '--rm', '--network=none', '--read-only', '--cap-drop=ALL',
            '--security-opt=no-new-privileges', '--tmpfs', '/tmp:rw,nosuid,nodev,size=256m',
            '-v', f'{fixture.resolve()}:/fixture:ro',
            '-v', f'{(output / "database").resolve()}:/database',
            '-v', f'{(output / "work").resolve()}:/work']
    for binary in ('ptero', 'pterobacter'):
        assert command(base + ['--entrypoint', binary, image, '--version']) == f'pterobacter {version}'
    inventory = command(base + ['--entrypoint', '/bin/sh', image, '-c',
                               'ls /usr/local/bin; test -s /etc/ssl/certs/ca-certificates.crt; '
                               'test -s /usr/share/doc/pterobacter/LICENSE-GPL-3.0.txt; '
                               'test -s /usr/share/doc/pterobacter/THIRD-PARTY-NOTICES.txt; '
                               '! command -v cargo; ! command -v g++; ! test -d /source'])
    assert inventory.splitlines() == ['ptero', 'pterobacter']
    (output / 'container-inspect.json').write_text(json.dumps(inspect, indent=2) + '\n')
    (output / 'runtime-libraries.txt').write_text(command(base + ['--entrypoint', '/bin/sh', image, '-c',
        'ldd /usr/local/bin/ptero; dpkg-query -W; sha256sum /usr/local/bin/*']) + '\n')
    pin = (fixture / 'catalogue.sha256').read_text().strip()
    selected = ['--catalogue', '/fixture/catalogue.json', '--catalogue-sha256', pin]
    def run(args, name, binary='ptero'):
        value = json.loads(command(base + ['--entrypoint', binary, image] + selected + args))
        (output / (name + '.json')).write_text(json.dumps(value, indent=2) + '\n')
        return value
    install = ['database', 'install', 'fixture-1', '--from-directory', '/fixture/archives']
    run(install, 'install')
    run(['database', 'verify', 'fixture-1'], 'verify')
    for binary in ('ptero', 'pterobacter'):
        result = run(['-D', 'fixture-1', '--threads', '2', '--report-dir', f'/work/{binary}',
                      '/fixture/fixed-query.fa'], binary, binary)
        assert result['execution']['status'] == 'complete'
        assert result['call_basis'] == 'ANIm_assisted'
        assert result['rescue_status'] == 'rescued'
        assert result['counts']['completed'] > 0
        assert result['mlst']['status'] not in ('failed', 'not_requested', 'not_run')
        for name in ('report.json', 'calls.tsv', 'evidence.tsv', 'report.md'):
            assert (output / 'work' / binary / name).is_file()
    typing = run(['-D', 'fixture-1', '--no-anim', '/fixture/typing-query.fa'], 'typing-positive')
    assert typing['mlst']['sequence_type'] == '5'
    fast = run(['-D', 'fixture-1', '--no-anim', '--no-mlst', '/fixture/fixed-query.fa'], 'opt-outs')
    assert fast['execution']['status'] == 'complete'
    reuse = run(['database', 'install', 'fixture-1'], 'offline-reuse')
    assert all(c['status'] == 'verified_reuse' for c in reuse['components'])
    archive = output / f'pterobacter-{version}-linux-amd64-image.tar.gz'
    with archive.open('wb') as raw, gzip.GzipFile(fileobj=raw, mode='wb', mtime=0) as zipped:
        process = subprocess.Popen(['docker', 'image', 'save', image], stdout=subprocess.PIPE)
        while block := process.stdout.read(1024 * 1024):
            zipped.write(block)
        if process.wait():
            raise RuntimeError('docker save failed')
    with archive.open('rb') as source:
        checksum = hashlib.file_digest(source, 'sha256').hexdigest()
    receipt = {'version': version, 'source_commit': commit, 'image_id': inspect['Id'],
               'architecture': 'linux/amd64', 'image_size_bytes': inspect['Size'],
               'archive': archive.name, 'archive_bytes': archive.stat().st_size,
               'archive_sha256': checksum, 'tests': 'non-root, read-only, network-disabled; managed install/verify/reuse; both aliases; native ANIm; default MLST; opt-outs; reports'}
    (output / 'container-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    (output / 'CONTAINER-SHA256SUMS').write_text(checksum + '  ' + archive.name + '\n')
    print(json.dumps(receipt, indent=2))


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--image', required=True)
    p.add_argument('--fixture', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--version', required=True)
    p.add_argument('--commit', required=True)
    a = p.parse_args()
    check(a.image, a.fixture, a.output, a.version, a.commit)
