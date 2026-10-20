"""Validate a release tag and source before any registry or release publication."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import tomllib


def output(args):
    return subprocess.check_output(args, text=True).strip()


def check(tag, publish=False):
    version = tomllib.loads(Path('Cargo.toml').read_text())['package']['version']
    commit = output(['git', 'rev-parse', 'HEAD'])
    if tag:
        if not re.fullmatch(r'v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?', tag):
            raise ValueError('Expected a version tag such as v0.1.0')
        if tag != 'v' + version:
            raise ValueError('Tag does not match Cargo package version')
        if output(['git', 'rev-parse', f'refs/tags/{tag}^{{commit}}']) != commit:
            raise ValueError('Checkout does not match the tag')
        subprocess.run(['git', 'merge-base', '--is-ancestor', commit, 'origin/main'], check=True)
    if publish:
        if not tag or version == '0.0.0':
            raise ValueError('Publication needs a real reviewed version tag')
        repository = output(['gh', 'repo', 'view', '--json', 'nameWithOwner,visibility'])
        repository = json.loads(repository)
        if repository['visibility'] != 'PUBLIC':
            raise ValueError('Repository is private: obtain author approval for source visibility before public distribution')
        runs = json.loads(output(['gh', 'run', 'list', '--repo', repository['nameWithOwner'],
                                  '--workflow', 'ci.yml', '--branch', 'main', '--commit', commit, '--event', 'push',
                                  '--status', 'success', '--json', 'headSha,conclusion']))
        if not any(r['headSha'] == commit and r['conclusion'] == 'success' for r in runs):
            raise ValueError('No successful main CI run for this exact source commit')
    return {'version': version, 'source_commit': commit, 'tag': tag, 'publish': publish}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag', default='')
    parser.add_argument('--publish', action='store_true')
    args = parser.parse_args()
    print(json.dumps(check(args.tag, args.publish)))
