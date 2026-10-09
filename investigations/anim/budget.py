"""Persistent conservative CPU admission for Linux development experiments.

Reserve the entire pair deadline times its CPU affinity width, plus two seconds
for process cleanup. Debit elapsed wall time times that width (an upper bound on
process-tree CPU), or the measured CPU if larger. Interrupted reservations stay
charged: a new invocation cannot silently forget unfinished work.
"""
import contextlib
import fcntl
import json
import os
import pathlib
import time
import uuid
import datetime


class BudgetExhausted(RuntimeError):
    pass


class Budget:
    def __init__(self, path, limit=7200):
        self.path = pathlib.Path(path)
        self.limit = limit
        self.cpu_limit = min(8, int(os.environ.get('SLURM_CPUS_PER_TASK', 8)))
        self.path.parent.mkdir(parents=True, exist_ok=True)

    @contextlib.contextmanager
    def transaction(self):
        with self.path.with_suffix('.lock').open('a') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            data = json.loads(self.path.read_text()) if self.path.exists() else dict(
                limit_seconds=self.limit, accounting='max(process-tree GNU CPU, elapsed wall * affinity CPUs); outstanding reservations charged in full', entries=[])
            if data['limit_seconds'] != self.limit:
                raise ValueError('ledger limit cannot change across invocations')
            yield data
            temporary = self.path.with_suffix('.tmp')
            temporary.write_text(json.dumps(data, indent=2) + '\n')
            os.replace(temporary, self.path)

    @staticmethod
    def charged(data):
        return sum(e.get('charged_seconds', e['reserved_seconds']) for e in data['entries'])

    def reserve(self, label, seconds, cpus, clock=time.monotonic):
        reservation = (seconds + 2) * cpus
        key = uuid.uuid4().hex
        with self.transaction() as data:
            if any(e.get('cleanup_bound_exceeded') for e in data['entries']):
                raise BudgetExhausted('prior cleanup exceeded reserved headroom; stop for review')
            if self.charged(data) + reservation > self.limit:
                raise BudgetExhausted('remaining CPU budget cannot accommodate a complete bounded pair')
            if sum(e['cpus'] for e in data['entries'] if e['status']=='running') + cpus > self.cpu_limit:
                raise BudgetExhausted('concurrent reservations exceed the allocation; defer this invocation')
            data['entries'].append(dict(id=key, label=label, reserved_seconds=reservation,
                                        cpus=cpus, status='running', pid=os.getpid(),
                                        started_utc=datetime.datetime.now(datetime.timezone.utc).isoformat()))
        return Lease(self, key, seconds, cpus, clock)


class Lease:
    def __init__(self, budget, key, seconds, cpus, clock):
        self.budget, self.key, self.cpus, self.clock = budget, key, cpus, clock
        self.start = clock()
        self.deadline = self.start + seconds
        self.cpu = 0

    def remaining(self):
        remaining = self.deadline - self.clock()
        if remaining <= 0:
            raise RuntimeError('reciprocal pair exceeded shared deadline')
        return remaining

    def finish(self, failed=False):
        with self.budget.transaction() as data:
            entry = next(e for e in data['entries'] if e['id'] == self.key)
            # Unknown/killed work is charged in full, including cleanup headroom.
            measured_bound = max(self.cpu, (self.clock() - self.start) * self.cpus)
            charge = max(entry['reserved_seconds'], measured_bound) if failed else measured_bound
            entry.update(status='failed' if failed else 'complete', charged_seconds=charge,
                         measured_cpu_seconds=self.cpu,
                         finished_utc=datetime.datetime.now(datetime.timezone.utc).isoformat())
            if charge > entry['reserved_seconds']:
                entry['cleanup_bound_exceeded'] = True

