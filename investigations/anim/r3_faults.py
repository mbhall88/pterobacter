"""Bounded Linux R3 process-containment checks on exposed synthetic inputs."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import time
from budget import Budget


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--fixtures', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--budget', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(exist_ok=False)
    cpu = min(os.sched_getaffinity(0))
    os.sched_setaffinity(0, {cpu})
    budget = Budget(args.budget)
    outcomes = []
    for name in ['empty-path', 'deadline', 'parent-killed']:
        lease = budget.reserve('fault/' + name, 90, 1)
        result = args.output / name
        argv = [str(args.binary), 'anim-pair', '--query', str(args.fixtures / 'genome_5mb.q.fa'),
                '--reference', str(args.fixtures / 'genome_5mb.r.fa'), '--output', str(result),
                '--timeout-seconds', '1' if name == 'deadline' else '90']
        env = dict(os.environ)
        if name == 'empty-path':
            env['PATH'] = ''
        with (args.output / (name + '.stdout')).open('wb') as stdout, (args.output / (name + '.stderr')).open('wb') as stderr:
            child = subprocess.Popen(argv, env=env, stdout=stdout, stderr=stderr)
            try:
                if name == 'parent-killed':
                    start = time.monotonic()
                    worker = None
                    while time.monotonic() - start < 10:
                        children = Path(f'/proc/{child.pid}/task/{child.pid}/children')
                        if children.exists() and children.read_text().strip():
                            worker = int(children.read_text().split()[0])
                            break
                        if child.poll() is not None:
                            raise RuntimeError('coordinator ended before parent-death check')
                        time.sleep(.001)
                    if worker is None:
                        raise RuntimeError('no native worker observed')
                    os.kill(child.pid, signal.SIGKILL)
                    status = child.wait(timeout=5)
                    for _ in range(500):
                        stat = Path(f'/proc/{worker}/stat')
                        if not stat.exists() or stat.read_text().split()[2] == 'Z':
                            break
                        time.sleep(.01)
                    else:
                        raise RuntimeError('native worker remained running after parent death')
                    outcome = {'case': name, 'exit_code': status, 'worker_pid': worker, 'worker_stopped': True}
                else:
                    status = child.wait(timeout=95)
                    report = json.loads((result / 'result.json').read_text())
                    expected = 'complete' if name == 'empty-path' else 'incomplete'
                    if report['status'] != expected:
                        raise RuntimeError(f'{name}: {report["status"]}, expected {expected}')
                    outcome = {'case': name, 'exit_code': status, 'status': report['status'], 'error': report['error']}
                    if name == 'empty-path' and status != 0:
                        raise RuntimeError('self-contained run failed')
                    if name == 'deadline' and (status == 0 or 'deadline' not in (report['error'] or '')):
                        raise RuntimeError('deadline was not recorded as failure')
                lease.finish()
                outcomes.append(outcome)
                print(json.dumps(outcome), flush=True)
            except BaseException:
                child.kill()
                child.wait()
                lease.finish(failed=True)
                raise
    (args.output / 'summary.json').write_text(json.dumps(outcomes, indent=2) + '\n')


if __name__ == '__main__':
    main()
