"""Execute the frozen development matrix. Outputs are research provenance."""
import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import hashlib
import json
import os
import pathlib
import signal
import subprocess
import threading
import time
from collections import defaultdict
from metrics import delta_metrics, paf_metrics


def fasta_lengths(path):
    records = {}
    current = None
    with path.open() as stream:
        for line in stream:
            line = line.strip()
            if not line:
                continue
            if line.startswith('>'):
                if current is not None and records[current] == 0:
                    raise ValueError('empty FASTA record')
                fields = line[1:].split()
                if not fields or fields[0] in records:
                    raise ValueError('invalid or duplicate FASTA ID')
                current = fields[0]
                records[current] = 0
            else:
                if current is None or not set(line.upper()) <= set('ACGTRYSWKMBDHVN'):
                    raise ValueError('invalid FASTA sequence')
                records[current] += len(line)
    if not records or not all(records.values()):
        raise ValueError('empty FASTA')
    return records


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Runner:
    def __init__(self, args):
        self.args = args
        self.active = set()
        self.lock = threading.Lock()

    def timed(self, argv, directory, label, stdout, timeout_seconds=1800):
        timing = directory / (label + '.time')
        stderr = directory / (label + '.stderr')
        full = ['/usr/bin/time', '-f', '%e\t%U\t%S\t%M\t%x', '-o', str(timing), *map(str, argv)]
        (directory / (label + '.argv.json')).write_text(json.dumps(full) + '\n')
        with stdout.open('wb') as out, stderr.open('wb') as err:
            child = subprocess.Popen(full, stdout=out, stderr=err, start_new_session=True)
            with self.lock:
                self.active.add(child.pid)
            peak = [0]
            stop = threading.Event()
            def sample_pair():
                while not stop.wait(.01):
                    peak[0] = max(peak[0], self.memory_sample([child.pid]))
            sampler = threading.Thread(target=sample_pair) if self.args.sample_pairs else None
            if sampler:
                sampler.start()
            try:
                code = child.wait(timeout=timeout_seconds)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
                raise RuntimeError('pair exceeded preregistered timeout')
            finally:
                stop.set()
                if sampler:
                    sampler.join()
                with self.lock:
                    self.active.discard(child.pid)
        # GNU time writes a diagnostic before its last record on failure.
        values = timing.read_text().splitlines()[-1].split('\t')
        return dict(argv=list(map(str, argv)), exit_code=code, wall_seconds=float(values[0]),
                    cpu_seconds=float(values[1]) + float(values[2]), peak_kib=int(values[3]), sampled_tree_peak_kib=peak[0] if sampler else None)

    def orientation(self, candidate, reference, query, directory):
        directory.mkdir(parents=True)
        ref_lengths, query_lengths = fasta_lengths(reference), fasta_lengths(query)
        if candidate == 'comparator':
            prefix = directory / 'native'
            first = self.timed([self.args.nucmer, '--mum', '-p', prefix, reference, query], directory, 'nucmer', directory / 'nucmer.stdout')
            receipts = [first]
            if first['exit_code'] == 0:
                receipts.append(self.timed([self.args.delta_filter, '-1', str(prefix) + '.delta'], directory, 'filter', directory / 'native.filter'))
            parser, raw = delta_metrics, directory / 'native.filter'
        elif candidate == 'embedded-mummer3':
            prefix = directory / 'native'
            receipts = [self.timed([self.args.mummer_probe, reference, query, prefix], directory, 'embedded', directory / 'embedded.stdout')]
            parser, raw = delta_metrics, directory / 'native.filter'
        else:
            receipts = [self.timed([self.args.minimap_probe, candidate, reference, query], directory, 'binding', directory / 'alignment.paf')]
            parser, raw = paf_metrics, directory / 'alignment.paf'
        result = dict(receipts=receipts, wall_seconds=sum(r['wall_seconds'] for r in receipts),
                      cpu_seconds=sum(r['cpu_seconds'] for r in receipts), peak_kib=max(r['peak_kib'] for r in receipts), sampled_tree_peak_kib=max((r['sampled_tree_peak_kib'] or 0) for r in receipts) if self.args.sample_pairs else None)
        if any(r['exit_code'] for r in receipts):
            result['metrics'] = dict(status='error')
        else:
            try:
                result['metrics'] = parser(raw, ref_lengths, query_lengths)
            except ValueError as error:
                result['metrics'] = dict(status='error', message=str(error))
            result['alignment_sha256'] = sha(raw)
            if parser == delta_metrics:
                result['normalized_delta_sha256'] = hashlib.sha256('\n'.join(raw.read_text().splitlines()[1:]).encode()).hexdigest()
        return result

    def pair(self, candidate, case, destination):
        reference, query = [self.args.fixtures / case[k] for k in ('reference', 'query')]
        result = dict(fixture=case['id'], candidate=candidate, stratum=case['stratum'])
        directory = destination / candidate / case['id']
        result['QR'] = self.orientation(candidate, reference, query, directory / 'QR')
        result['RQ'] = self.orientation(candidate, query, reference, directory / 'RQ')
        result['wall_seconds'] = result['QR']['wall_seconds'] + result['RQ']['wall_seconds']
        result['cpu_seconds'] = result['QR']['cpu_seconds'] + result['RQ']['cpu_seconds']
        result['sampled_tree_peak_kib'] = max((result[o]['sampled_tree_peak_kib'] or 0) for o in ['QR', 'RQ']) if self.args.sample_pairs else None
        result['peak_kib'] = max(result['QR']['peak_kib'], result['RQ']['peak_kib'])
        (directory / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
        return result

    def memory_sample(self, pids=None):
        with self.lock:
            pending = list(self.active) if pids is None else list(pids)
        seen = set()
        total = 0
        while pending:
            pid = pending.pop()
            if pid in seen:
                continue
            seen.add(pid)
            try:
                status = pathlib.Path(f'/proc/{pid}/status').read_text()
                total += next((int(line.split()[1]) for line in status.splitlines() if line.startswith('VmRSS:')), 0)
                pending.extend(map(int, pathlib.Path(f'/proc/{pid}/task/{pid}/children').read_text().split()))
            except (FileNotFoundError, ProcessLookupError):
                pass
        return total

    def panel(self, candidate, cases, destination):
        stop = threading.Event()
        peak = [0]
        def sample():
            while not stop.wait(.05):
                peak[0] = max(peak[0], self.memory_sample())
        sampler = threading.Thread(target=sample)
        sampler.start()
        start = time.monotonic()
        try:
            with ThreadPoolExecutor(max_workers=8) as pool:
                futures = [pool.submit(self.pair, candidate, case, destination) for case in cases]
                results = [future.result() for future in as_completed(futures)]
        finally:
            stop.set()
            sampler.join()
        summary = dict(candidate=candidate, pairs=len(results), workers=8, wall_seconds=time.monotonic() - start,
                       cpu_seconds=sum(r['cpu_seconds'] for r in results),
                       sampled_worker_peak_kib=peak[0], sampling_interval_seconds=.05,
                       maximum_pair_peak_kib=max(r['peak_kib'] for r in results),
                       failed_orientations=sum(r[o]['metrics']['status'] == 'error' for r in results for o in ('QR', 'RQ')))
        (destination / candidate / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
        return summary


def main():
    parser = argparse.ArgumentParser()
    for name in ('fixtures', 'output', 'nucmer', 'delta-filter', 'mummer-probe', 'minimap-probe'):
        parser.add_argument('--' + name, required=True, type=pathlib.Path)
    parser.add_argument('--panel', action='store_true')
    parser.add_argument('--sample-pairs', action='store_true')
    parser.add_argument('--candidates', nargs='+', choices=['comparator', 'embedded-mummer3', 'asm5', 'asm10', 'asm20'], default=['comparator', 'embedded-mummer3', 'asm5', 'asm10', 'asm20'])
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    manifest = json.loads((args.fixtures / 'manifest.json').read_text())
    # Ensure the generator manifest still binds every input byte before measurement.
    for name, expected in manifest['sha256'].items():
        if sha(args.fixtures / name) != expected:
            raise ValueError('fixture hash mismatch: ' + name)
    runner = Runner(args)
    candidates = args.candidates
    results = []
    for candidate in candidates:
        for case in manifest['cases']:
            if case['valid']:
                result = runner.pair(candidate, case, args.output / 'matrix')
                results.append(result)
                print(candidate, case['id'], result['QR']['metrics']['status'], result['RQ']['metrics']['status'], flush=True)
            else:
                directory = args.output / 'invalid' / candidate / case['id']
                directory.mkdir(parents=True)
                ref, query = [args.fixtures / case[k] for k in ('reference', 'query')]
                if candidate == 'comparator':
                    # The comparator's input guard is the supported integrity contract.
                    try:
                        fasta_lengths(ref)
                        fasta_lengths(query)
                        raise AssertionError('invalid fixture accepted')
                    except (ValueError, FileNotFoundError) as error:
                        result = dict(fixture=case['id'], candidate=candidate, status='invalid_input', message=str(error), guard='development contract validation before native execution')
                else:
                    argv = [args.mummer_probe, ref, query, directory / 'x'] if candidate == 'embedded-mummer3' else [args.minimap_probe, candidate, ref, query]
                    receipt = runner.timed(argv, directory, 'invalid', directory / 'stdout')
                    result = dict(fixture=case['id'], candidate=candidate, status='invalid_input' if receipt['exit_code'] != 0 else 'unexpected_success', receipt=receipt)
                (directory / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    (args.output / 'matrix.json').write_text(json.dumps(results, indent=2) + '\n')
    if args.panel:
        summaries = []
        for candidate in candidates:
            summary = runner.panel(candidate, manifest['panel'], args.output / 'panel')
            summaries.append(summary)
            print(json.dumps(summary), flush=True)
            if sum(s['cpu_seconds'] for s in summaries) > 7200:
                raise RuntimeError('predeclared alignment CPU budget exceeded')
        (args.output / 'panel.json').write_text(json.dumps(summaries, indent=2) + '\n')

if __name__ == '__main__':
    main()
