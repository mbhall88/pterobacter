"""Replay the approved R3 implementation against retained synthetic comparator artifacts."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time
from metrics import delta_metrics
from budget import Budget


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def lengths(path):
    records = {}
    for line in path.read_text().splitlines():
        if line.startswith('>'):
            key = line[1:].split()[0]
            records[key] = 0
        else:
            records[key] += len(line.strip())
    return records


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--fixtures', required=True, type=Path)
    parser.add_argument('--comparator', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    args.output.mkdir(exist_ok=False)
    manifest = json.loads((args.fixtures / 'manifest.json').read_text())
    cpus = sorted(os.sched_getaffinity(0))[:8]
    os.sched_setaffinity(0, cpus)
    start = time.monotonic()
    attempts = []
    budget = Budget(args.output / "alignment-budget.json")

    def run(item, group):
        lease = budget.reserve(group + "/" + item["id"], 90, 1)
        try:
            row = measure(item, group)
            lease.cpu = row["cpu_seconds"]
            lease.finish(failed=not row["matched"])
            return row
        except BaseException:
            lease.finish(failed=True)
            raise

    def measure(item, group):
        directory = args.output / group / item['id']
        directory.parent.mkdir(exist_ok=True)
        query, reference = [args.fixtures / item[k] for k in ['query', 'reference']]
        cpu = cpus[int(hashlib.sha256(item['id'].encode()).hexdigest(), 16) % len(cpus)]
        argv = ['taskset', '-c', str(cpu), str(args.binary), 'anim-pair', '--query', str(query), '--reference', str(reference),
                '--query-id', 'Q', '--reference-id', 'R', '--timeout-seconds', '90',
                '--memory-mib', '6144', '--output', str(directory)]
        receipt = subprocess.run(argv, capture_output=True, timeout=95)
        (directory / 'coordinator.stdout').write_bytes(receipt.stdout)
        (directory / 'coordinator.stderr').write_bytes(receipt.stderr)
        report = json.loads((directory / 'result.json').read_text())
        row = {'id': item['id'], 'group': group, 'exit_code': receipt.returncode,
               'status': report['status'], 'matched': False, 'differences': [],
               'cpu_seconds': sum(o['receipt']['cpu_seconds'] or 0 for o in report['orientations'].values() if o['receipt'])}
        if not item['valid']:
            row['matched'] = receipt.returncode != 0 and report['status'] == 'incomplete'
            return row
        if receipt.returncode != 0 or report['status'] != 'complete':
            row['differences'].append({'error': report['error']})
            return row
        fields = ['status', 'D', 'E', 'blocks', 'reference_covered', 'query_covered', 'reference_length', 'query_length']
        qlength, rlength = lengths(query), lengths(reference)
        for direction in ['QR', 'RQ']:
            r, q = (rlength, qlength) if direction == 'QR' else (qlength, rlength)
            comparator = args.comparator / group / 'comparator' / item['id'] / direction / 'native.filter'
            expected = delta_metrics(comparator, r, q)
            if direction == 'RQ':
                for kind in ['covered', 'length']:
                    expected['reference_' + kind], expected['query_' + kind] = expected['query_' + kind], expected['reference_' + kind]
            actual = report['orientations'][direction]['metrics']
            for field in fields:
                if expected[field] != actual[field]:
                    row['differences'].append({'direction': direction, 'field': field,
                                               'expected': expected[field], 'actual': actual[field]})
            row.setdefault('comparator_sha256', {})[direction] = digest(comparator)
        row['matched'] = not row['differences']
        row['query_sha256'], row['reference_sha256'] = digest(query), digest(reference)
        return row

    # Preserve matrix failures before starting the panel. Never rerun an attempt.
    for item in manifest['cases']:
        row = run(item, 'matrix' if item['valid'] else 'invalid')
        attempts.append(row)
        print(json.dumps(row), flush=True)
    with ThreadPoolExecutor(max_workers=8) as pool:
        for row in pool.map(lambda item: run(item, 'panel'), manifest['panel']):
            attempts.append(row)
            print(json.dumps(row), flush=True)
    summary = {'binary_sha256': digest(args.binary), 'fixture_manifest_sha256': digest(args.fixtures / 'manifest.json'),
               'cpu_affinity': cpus, 'workers': 8, 'wall_seconds': time.monotonic() - start,
               'cpu_seconds': sum(r['cpu_seconds'] for r in attempts), 'attempts': attempts,
               'matched': sum(r['matched'] for r in attempts), 'total': len(attempts)}
    (args.output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    if summary['cpu_seconds'] > 7200 or summary['matched'] != summary['total']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
