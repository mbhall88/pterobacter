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
import sys
import queue
from collections import defaultdict
from metrics import delta_metrics, paf_metrics
from budget import Budget, BudgetExhausted


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
        self.local = threading.local()
        self.budget = Budget(args.budget_ledger) if getattr(args, 'budget_ledger', None) else None
        self.workers = getattr(args, 'workers', 8)
        self.threads = getattr(args, 'threads', 1)
        self.pair_seconds = getattr(args, 'pair_seconds', 1800)
        allocation = int(os.environ.get('SLURM_CPUS_PER_TASK', self.workers * self.threads))
        cpus = sorted(os.sched_getaffinity(0))[:allocation]
        if self.workers * self.threads > len(cpus):
            raise ValueError('worker/thread allocation exceeds available CPU allocation')
        self.slots = queue.Queue()
        for start in range(0, self.workers * self.threads, self.threads):
            self.slots.put(cpus[start:start + self.threads])

    def timed(self, argv, directory, label, stdout, timeout_seconds=1800):
        lease = getattr(self.local, 'lease', None)
        if self.budget and lease is None:
            # Invalid-input/native diagnostic calls also use the same ledger.
            return self.standalone(argv, directory, label, stdout, min(timeout_seconds, self.pair_seconds))
        remaining = min(timeout_seconds, lease.remaining()) if lease else timeout_seconds
        timing = directory / (label + '.time')
        stderr = directory / (label + '.stderr')
        native = list(map(str, argv))
        if lease:
            native = [sys.executable, str(pathlib.Path(__file__).with_name('bounded_exec.py')),
                      ','.join(map(str, self.local.cpus)), str(getattr(self.args, 'memory_kib', 8 * 1024 * 1024)), *native]
        full = ['/usr/bin/time', '-f', '%e\t%U\t%S\t%M\t%x', '-o', str(timing), *native]
        (directory / (label + '.argv.json')).write_text(json.dumps(full) + '\n')
        with stdout.open('wb') as out, stderr.open('wb') as err:
            if lease:
                lease.remaining()  # include command/output setup in the deadline
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
                end = min(time.monotonic() + remaining, lease.deadline) if lease else time.monotonic() + remaining
                while True:
                    wait = min(.01, end - time.monotonic())
                    if wait <= 0:
                        raise RuntimeError('reciprocal pair exceeded shared timeout')
                    if self.memory_sample([child.pid]) > getattr(self.args, 'memory_kib', 8 * 1024 * 1024):
                        raise RuntimeError('pair process tree exceeded memory ceiling')
                    try:
                        code = child.wait(timeout=wait)
                        break
                    except subprocess.TimeoutExpired:
                        pass
            except RuntimeError as error:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
                (directory / (label + '.failure.json')).write_text(json.dumps(dict(argv=list(map(str, argv)), error=str(error), status='error')) + '\n')
                raise
            finally:
                stop.set()
                if sampler:
                    sampler.join()
                with self.lock:
                    self.active.discard(child.pid)
        # GNU time writes a diagnostic before its last record on failure.
        values = timing.read_text().splitlines()[-1].split('\t')
        if lease:
            lease.cpu += float(values[1]) + float(values[2])
        return dict(argv=list(map(str, argv)), exit_code=code, wall_seconds=float(values[0]),
                    cpu_seconds=float(values[1]) + float(values[2]), peak_kib=int(values[3]), sampled_tree_peak_kib=peak[0] if sampler else None)

    def standalone(self, argv, directory, label, stdout, seconds):
        cpus = self.slots.get()
        lease = None
        failed = True
        try:
            lease = self.budget.reserve(str(directory / label), seconds, self.threads)
            self.local.lease, self.local.cpus = lease, cpus
            result = self.timed(argv, directory, label, stdout, seconds)
            failed = False
            return result
        finally:
            if lease:
                lease.finish(failed)
            self.local.lease = None
            self.slots.put(cpus)

    def orientation(self, candidate, reference, query, directory):
        directory.mkdir(parents=True)
        ref_lengths, query_lengths = fasta_lengths(reference), fasta_lengths(query)
        if candidate in ('comparator', 'cli-mummer4'):
            prefix = directory / 'native'
            flags = ['--threads', str(self.threads)] if candidate == 'cli-mummer4' else []
            first = self.timed([self.args.nucmer, '--mum', *flags, '-p', prefix, reference, query], directory, 'nucmer', directory / 'nucmer.stdout')
            receipts = [first]
            if first['exit_code'] == 0:
                receipts.append(self.timed([self.args.delta_filter, '-1', str(prefix) + '.delta'], directory, 'filter', directory / 'native.filter'))
            parser, raw = delta_metrics, directory / 'native.filter'
        elif candidate in ('embedded-mummer3', 'embedded-mummer4'):
            prefix = directory / 'native'
            extra = [str(self.threads)] if candidate == 'embedded-mummer4' else []
            receipts = [self.timed([self.args.mummer_probe, reference, query, prefix, *extra], directory, 'embedded', directory / 'embedded.stdout')]
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
        cpus = self.slots.get()
        lease = None
        failed = True
        try:
            if self.budget:
                lease = self.budget.reserve(str(directory), self.pair_seconds, self.threads)
            self.local.lease, self.local.cpus = lease, cpus
            # One deadline includes input parsing, both orientations and all stages.
            # Even legacy callers without a ledger share the pair deadline.
            if lease is None:
                from budget import Lease
                self.local.lease = Lease(None, None, self.pair_seconds, self.threads, time.monotonic)
            for direction, ref, qry in [('QR', reference, query), ('RQ', query, reference)]:
                self.local.lease.remaining()
                result[direction] = self.orientation(candidate, ref, qry, directory / direction)
            failed = False
        except (RuntimeError, ValueError, OSError) as error:
            for direction in ('QR', 'RQ'):
                if direction not in result:
                    result[direction] = dict(metrics=dict(status='not_run' if isinstance(error, BudgetExhausted) else 'error', message=str(error)),
                                             wall_seconds=0, cpu_seconds=0, peak_kib=0, sampled_tree_peak_kib=None)
        finally:
            if lease:
                lease.finish(failed)
            self.local.lease = None
            self.slots.put(cpus)
        directory.mkdir(parents=True, exist_ok=True)
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
                for children in pathlib.Path(f'/proc/{pid}/task').glob('*/children'):
                    pending.extend(map(int, children.read_text().split()))
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
            # Submit only a bounded batch. Admission happens before any native
            # work, and failed admission stops subsequent batch scheduling.
            results = []
            with ThreadPoolExecutor(max_workers=self.workers) as pool:
                for start_at in range(0, len(cases), self.workers):
                    batch = cases[start_at:start_at + self.workers]
                    futures = [pool.submit(self.pair, candidate, case, destination) for case in batch]
                    completed = [future.result() for future in as_completed(futures)]
                    results.extend(completed)
                    if any(r[o]['metrics']['status'] == 'not_run' for r in completed for o in ('QR', 'RQ')):
                        break
        finally:
            stop.set()
            sampler.join()
        summary = dict(candidate=candidate, pairs=len(results), planned_pairs=len(cases), workers=self.workers, wall_seconds=time.monotonic() - start,
                       cpu_seconds=sum(r['cpu_seconds'] for r in results),
                       sampled_worker_peak_kib=peak[0], sampling_interval_seconds=.05,
                       maximum_pair_peak_kib=max(r['peak_kib'] for r in results),
                       failed_orientations=sum(r[o]['metrics']['status'] not in ('aligned', 'no_alignment') for r in results for o in ('QR', 'RQ')))
        (destination / candidate / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
        return summary


def main():
    parser = argparse.ArgumentParser()
    for name in ('fixtures', 'output', 'nucmer', 'delta-filter', 'mummer-probe', 'minimap-probe'):
        parser.add_argument('--' + name, required=True, type=pathlib.Path)
    parser.add_argument('--panel', action='store_true')
    parser.add_argument('--sample-pairs', action='store_true')
    parser.add_argument('--budget-ledger', required=True, type=pathlib.Path, help='same persistent ledger for every pass in an experiment')
    parser.add_argument('--pair-seconds', type=float, default=1800)
    parser.add_argument('--workers', type=int, default=8)
    parser.add_argument('--threads', type=int, default=1)
    parser.add_argument('--memory-kib', type=int, default=6 * 1024 * 1024)
    parser.add_argument('--candidates', nargs='+', choices=['comparator', 'embedded-mummer3', 'cli-mummer4', 'embedded-mummer4', 'asm5', 'asm10', 'asm20'], default=['comparator', 'embedded-mummer3', 'asm5', 'asm10', 'asm20'])
    args = parser.parse_args()
    if not 0 < args.pair_seconds <= 1800 or not 1 <= args.workers <= 8 or args.threads < 1:
        parser.error('pair deadline <=1800 seconds; 1..8 workers; positive threads required')
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
                if any(result[o]['metrics']['status'] == 'not_run' for o in ('QR', 'RQ')):
                    break
            else:
                directory = args.output / 'invalid' / candidate / case['id']
                directory.mkdir(parents=True)
                ref, query = [args.fixtures / case[k] for k in ('reference', 'query')]
                if candidate in ('comparator', 'cli-mummer4'):
                    # The comparator's input guard is the supported integrity contract.
                    try:
                        fasta_lengths(ref)
                        fasta_lengths(query)
                        raise AssertionError('invalid fixture accepted')
                    except (ValueError, FileNotFoundError) as error:
                        result = dict(fixture=case['id'], candidate=candidate, status='invalid_input', message=str(error), guard='development contract validation before native execution')
                else:
                    argv = [args.mummer_probe, ref, query, directory / 'x', *([str(args.threads)] if candidate == 'embedded-mummer4' else [])] if candidate in ('embedded-mummer3', 'embedded-mummer4') else [args.minimap_probe, candidate, ref, query]
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
        (args.output / 'panel.json').write_text(json.dumps(summaries, indent=2) + '\n')

if __name__ == '__main__':
    main()
