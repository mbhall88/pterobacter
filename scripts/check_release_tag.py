"""Validate a release tag and source before any registry or release publication."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import tomllib
import sys
import time


def output(args):
    return subprocess.check_output(args, text=True).strip()


def wait_for_source_ci(repository, commit, wait_seconds=0):
    """Wait only for the release commit's main push CI, never a PR check."""
    deadline = time.monotonic() + wait_seconds
    while True:
        runs = json.loads(output(['gh', 'run', 'list', '--repo', repository,
                                  '--workflow', 'ci.yml', '--branch', 'main', '--commit', commit,
                                  '--event', 'push', '--json', 'headSha,status,conclusion']))
        matching = [run for run in runs if run['headSha'] == commit]
        if any(run['status'] == 'completed' and run['conclusion'] == 'success' for run in matching):
            return
        if matching and all(run['status'] == 'completed' for run in matching):
            raise ValueError('Main CI did not pass for this exact source commit')
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise ValueError('No successful main CI run for this exact source commit; wait expired')
        print(f'Waiting for main CI on {commit}', file=sys.stderr, flush=True)
        time.sleep(min(15, remaining))


def check(tag, publish=False, wait_seconds=0):
    version = tomllib.loads(Path('Cargo.toml').read_text())['package']['version']
    commit = output(['git', 'rev-parse', 'HEAD'])
    if tag:
        if not re.fullmatch(r'v?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?', tag):
            raise ValueError('Expected a version tag such as 0.1.0')
        if tag.removeprefix('v') != version:
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
        wait_for_source_ci(repository['nameWithOwner'], commit, wait_seconds)
    return {'version': version, 'source_commit': commit, 'tag': tag, 'publish': publish}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag', default='')
    parser.add_argument('--publish', action='store_true')
    parser.add_argument('--wait-for-ci', type=int, default=0, metavar='SECONDS',
                        help='Maximum wait for exact-commit main CI before publication')
    args = parser.parse_args()
    if args.wait_for_ci < 0 or (args.wait_for_ci and not args.publish):
        parser.error('--wait-for-ci requires --publish and a non-negative duration')
    print(json.dumps(check(args.tag, args.publish, args.wait_for_ci)))
