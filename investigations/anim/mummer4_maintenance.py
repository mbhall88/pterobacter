"""One bounded exploratory run; retain upstream builds and carry prior charges."""
import argparse
import datetime
import difflib
import hashlib
import json
import os
from pathlib import Path
import shutil
from types import SimpleNamespace
import subprocess

from budget import Budget
from compare_mummer4 import canonical_delta, compare
from measure import Runner, fasta_lengths
from metrics import delta_metrics

HERE = Path(__file__).resolve().parent


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n')


def replace_once(text, old, new):
    if text.count(old) != 1:
        raise ValueError('unexpected pinned source: ' + old)
    return text.replace(old, new, 1)


def command(argv, run, label, cwd, env=None):
    save(run/'logs'/f'{label}.argv.json', list(map(str, argv)))
    with (run/'logs'/f'{label}.log').open('w') as output:
        subprocess.run(argv, cwd=cwd, env=env, stdout=output,
                       stderr=subprocess.STDOUT, check=True)


def runner(run, fixtures, nucmer, filter_path, probe, width=1):
    return Runner(SimpleNamespace(fixtures=fixtures, nucmer=nucmer,
        delta_filter=filter_path, mummer_probe=probe, budget_ledger=run/'outputs/budget.json',
        workers=1, threads=width, pair_seconds=90, memory_kib=6*1024*1024,
        sample_pairs=True))


def positive_score_record(path, reference, query):
    record = delta_metrics(path, fasta_lengths(reference), fasta_lengths(query))
    similarity_errors = sum(int(block[1].split()[5]) for block in canonical_delta(path))
    record['similarity_errors'] = similarity_errors
    record['positive_score_identity'] = (100*(record['D']-2*similarity_errors)/record['D']
                                         if record['D'] else None)
    return record


def main():
    parser = argparse.ArgumentParser()
    for name in ('run', 'prior-run', 'original-run', 'cli3'):
        parser.add_argument('--'+name, type=Path, required=True)
    parser.add_argument('--resume-build', action='store_true', help='resume only the first failed build before any measurements')
    args = parser.parse_args()
    run, prior, original = args.run.resolve(), args.prior_run.resolve(), args.original_run.resolve()
    if not args.resume_build:
        run.mkdir(parents=True, exist_ok=False)
        for folder in ('inputs', 'logs', 'outputs', 'build'):
            (run/folder).mkdir()
        fixtures = original/'inputs/synthetic-fixtures'
        manifest = json.loads((fixtures/'manifest.json').read_text())
        for name, expected in manifest['sha256'].items():
            if sha(fixtures/name) != expected:
                raise ValueError('fixture bytes changed: '+name)
        prior_ledger = prior/'outputs/budget.json'
        data = json.loads(prior_ledger.read_text())
        if any(entry['status'] == 'running' for entry in data['entries']):
            raise ValueError('prior ledger has outstanding reservations')
        shutil.copy2(prior_ledger, run/'outputs/budget.json')
        shutil.copy2(HERE/'mummer4-maintenance-protocol.md', run/'inputs/protocol.md')
        source = run/'vendor/mummer4-preserve-n'
        upstream = prior/'vendor/mummer-4.0.1'
        shutil.copytree(upstream, source)
        changes = {}
        path = source/'src/umd/nucmer.cc'
        changes[path] = replace_once(path.read_text(), '  replace_n_random_letter(sequence);',
                                    '  // Exploratory profile: preserve reference ambiguity bases.')
        path = source/'src/tigr/delta.cc'
        text = path.read_text()
        start = text.index('ostream & DeltaGraph_t::outputDelta (ostream & out)')
        end = text.index('\n}', start)+2
        changes[path] = text[:start]+text[start:end].replace('cout', 'out')+text[end:]
        path = source/'include/mummer/nucmer.hpp'
        changes[path] = replace_once(path.read_text(), 'trampoline_align_file<AlignmentOut>',
                                    'trampoline_align_file<sequence_parser, AlignmentOut>')
        patch = []
        for path, text in changes.items():
            old = path.read_text()
            relative = str(path.relative_to(source))
            patch.extend(difflib.unified_diff(old.splitlines(True), text.splitlines(True),
                                            fromfile='a/'+relative, tofile='b/'+relative))
            path.write_text(text)
        (run/'inputs/upstream.patch').write_text(''.join(patch))
        probe = run/'source/probe'
        shutil.copytree(HERE/'mummer4-probe', probe, ignore=shutil.ignore_patterns('target'))
        shared = run/'source/probes/src'
        shared.mkdir(parents=True)
        shutil.copy2(HERE/'probes/src/lib.rs', shared/'lib.rs')
        bridge = (probe/'bridge.cc').read_text()
        start, end = bridge.index('struct CoutRedirect'), bridge.index('extern "C"')
        bridge = bridge[:start]+bridge[end:]
        bridge = replace_once(bridge, '      std::lock_guard<std::mutex> lock(rng_mutex);\n      std::srand(1);\n      mummer::nucmer::Options options;', '      mummer::nucmer::Options options;')
        start = bridge.index('    // v4.0.1 align_file')
        end = bridge.index('    output.close();', start)
        bridge = bridge[:start]+'    aligner->align_file(query, print, threads);\n'+bridge[end:]
        start = bridge.index('      // v4.0.1 outputDelta')
        end = bridge.index('      graph.outputDelta(filtered_output);', start)
        bridge = bridge[:start]+bridge[end:]
        bridge = bridge.replace('// v4.0.1 reference-N replacement and delta LIS tie-breaking use libc rand().',
                                '// Exploratory profile: only delta LIS tie-breaking uses libc rand().')
        (probe/'bridge.cc').write_text(bridge)
        shutil.copy2(HERE/'mummer4-maintenance-driver.cc', run/'source/driver.cc')
        freeze = dict(frozen_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
            source_pin='v4.0.1 eb734606f2d516f42a0e0dce7a116bfb88ec1ebf',
            prior_run=str(prior), original_run=str(original), fixtures=str(fixtures),
            ledger_predecessor_sha256=sha(prior_ledger), inherited_charge=Budget.charged(data),
            protocol_sha256=sha(run/'inputs/protocol.md'), patch_sha256=sha(run/'inputs/upstream.patch'),
            fixture_manifest_sha256=sha(fixtures/'manifest.json'),
            source_hashes={str(p.relative_to(source)): sha(p) for p in source.rglob('*') if p.is_file()},
            tool_hashes={str(p.relative_to(HERE)): sha(p) for p in HERE.rglob('*')
                         if p.is_file() and p.suffix in ('.py', '.cc', '.rs', '.toml', '.md')})
        save(run/'inputs/freeze.json', freeze)
        print('Frozen protocol and three upstream changes; prior charges carried forward.', flush=True)
    else:
        freeze = json.loads((run/'inputs/freeze.json').read_text())
        fixtures = Path(freeze['fixtures'])
        manifest = json.loads((fixtures/'manifest.json').read_text())
        prior_ledger = prior/'outputs/budget.json'
        data = json.loads(prior_ledger.read_text())
        if (run/'outputs/cli-mummer4').exists():
            raise ValueError('build resume cannot repeat measured alignments')
        source = run/'vendor/mummer4-preserve-n'
        probe = run/'source/probe'
    native = run/'build/native'
    native.mkdir(exist_ok=args.resume_build)
    if not args.resume_build:
        command([str(source/'configure'), '--disable-shared', '--enable-static',
                 '--disable-openmp', '--without-swig', '--with-pic',
                 'CXXFLAGS=-O3 -DNDEBUG -fPIC'], run, 'configure', native)
        command(['make', '-j4', 'nucmer', 'delta-filter'], run, 'make', native)
    env = dict(os.environ, MUMMER4_SOURCE=str(source), MUMMER4_BUILD=str(native),
               CARGO_TARGET_DIR=str(run/'build/rust'))
    command(['cargo', 'build', '--locked', '--release'], run, 'cargo-repair1' if args.resume_build else 'cargo', probe, env)
    bridge_obj = next((run/'build/rust/release/build').glob('*/out/0.o'))
    command(['g++', '-std=c++17', '-O3', '-pthread', str(run/'source/driver.cc'),
             str(bridge_obj), str(bridge_obj.with_name('1.o')),
             str(native/'.libs/libumdmummer.a'), '-o', str(run/'build/driver')],
            run, 'driver-build', run)
    executable = run/'build/rust/release/mummer4-feasibility-probe'
    save(run/'inputs/binaries.json', {str(p): sha(p) for p in
         (native/'nucmer', native/'delta-filter', executable, run/'build/driver',
          args.cli3/'nucmer', args.cli3/'delta-filter')})
    for candidate in ('cli-mummer4', 'embedded-mummer4'):
        command(['python3', str(HERE/'measure.py'), '--fixtures', str(fixtures),
                 '--output', str(run/'outputs'/candidate), '--nucmer', str(native/'nucmer'),
                 '--delta-filter', str(native/'delta-filter'), '--mummer-probe', str(executable),
                 '--minimap-probe', '/not-used', '--budget-ledger', str(run/'outputs/budget.json'),
                 '--pair-seconds', '90', '--workers', '1', '--threads', '1', '--sample-pairs',
                 '--candidates', candidate], run, candidate, run)
        results = json.loads((run/'outputs'/candidate/'matrix.json').read_text())
        planned = sum(case['valid'] for case in manifest['cases'])
        if len(results) != planned or any(result[o]['metrics']['status'] not in
                ('aligned', 'no_alignment') for result in results for o in ('QR', 'RQ')):
            raise RuntimeError('matrix failed or incomplete; stop without further diagnostics')
        invalid = list((run/'outputs'/candidate/'invalid'/candidate).glob('*/result.json'))
        if len(invalid) != 4 or any(json.loads(p.read_text())['status'] != 'invalid_input' for p in invalid):
            raise RuntimeError('invalid-input rejection failed; stop')
        print('Completed modified matrix: '+candidate, flush=True)
    cli_matrix = run/'outputs/cli-mummer4/matrix/cli-mummer4'
    library_matrix = run/'outputs/embedded-mummer4/matrix/embedded-mummer4'
    compare(original/'outputs/first-pass/matrix/comparator', cli_matrix, fixtures,
            run/'outputs/vs-mummer3.json')
    compare(prior/'outputs/cli4-matrix/matrix/cli-mummer4', cli_matrix, fixtures,
            run/'outputs/vs-upstream4.json')
    compare(cli_matrix, library_matrix, fixtures, run/'outputs/library-vs-cli.json')
    case = next(c for c in manifest['cases'] if c['id'] == 'ambiguous')
    same = dict(case, id='ambiguous-self', reference=case['query'])
    both_n = []
    for label, candidate, cli, filter_path in (
            ('mummer3', 'comparator', args.cli3/'nucmer', args.cli3/'delta-filter'),
            ('upstream4', 'cli-mummer4', prior/'build/cli/nucmer', prior/'build/cli/delta-filter'),
            ('modified4', 'cli-mummer4', native/'nucmer', native/'delta-filter')):
        result = runner(run, fixtures, cli, filter_path, executable).pair(
            candidate, same, run/'outputs/both-n'/label)
        for orientation in ('QR', 'RQ'):
            path = run/'outputs/both-n'/label/candidate/'ambiguous-self'/orientation/'native.filter'
            both_n.append(dict(route=label, orientation=orientation,
                               record=positive_score_record(path, fixtures/case['query'], fixtures/case['query'])))
    save(run/'outputs/both-n.json', both_n)
    checks = []
    for width in (1, 8):
        directory = run/'outputs'/f'in-process-{width}'
        directory.mkdir()
        jobs, details = [], []
        for fixture in ('fragmented', 'repeat_reference', 'ambiguous', 'genome_5mb'):
            case = next(c for c in manifest['cases'] if c['id'] == fixture)
            for orientation in ('QR', 'RQ'):
                ref, qry = [fixtures/case[k] for k in ('reference', 'query')]
                if orientation == 'RQ': ref, qry = qry, ref
                for repeat in range(2):
                    prefix = directory/f'{fixture}-{orientation}-{repeat}'
                    jobs.append('\t'.join(map(str, (ref, qry, prefix))))
                    details.append((fixture, orientation, prefix, ref, qry))
        jobs_path = directory/'jobs.tsv'
        jobs_path.write_text('\n'.join(jobs)+'\n')
        receipt = runner(run, fixtures, native/'nucmer', native/'delta-filter',
                         executable, width).standalone([run/'build/driver', jobs_path, str(width)],
                         directory, 'batch', directory/'stdout', 90, candidate='embedded-mummer4')
        receipts = (directory/'stdout').read_text().splitlines()
        if receipt['exit_code'] or len(receipts) != 16 or any(line.split('\t')[1] != '0' for line in receipts):
            raise RuntimeError('in-process diagnostic failed: retained batch receipts')
        for fixture, orientation, prefix, ref, qry in details:
            path = prefix.with_suffix('.filter')
            record = positive_score_record(path, ref, qry)
            baseline = library_matrix/fixture/orientation/'native.filter'
            checks.append(dict(workers=width, fixture=fixture, orientation=orientation,
                prefix=str(prefix), record=record, filtered_exact=canonical_delta(path)==canonical_delta(baseline),
                unfiltered_exact=canonical_delta(prefix.with_suffix('.delta'))==
                    canonical_delta(baseline.with_suffix('.delta'))))
        save(directory/'receipt.json', receipt)
        print('Completed shared-process batch: '+str(width)+' workers', flush=True)
    save(run/'outputs/in-process.json', checks)
    if sha(prior_ledger) != freeze['ledger_predecessor_sha256']:
        raise RuntimeError('prior ledger changed')
    final = json.loads((run/'outputs/budget.json').read_text())
    save(run/'outputs/summary.json', dict(
        inherited_charge=freeze['inherited_charge'], total_charge=Budget.charged(final),
        new_charge=Budget.charged(final)-freeze['inherited_charge'],
        measured_new_cpu=sum(e.get('measured_cpu_seconds', 0) for e in final['entries'][len(data['entries']):]),
        outstanding=sum(e['status']=='running' for e in final['entries']),
        cleanup_breaches=sum(bool(e.get('cleanup_bound_exceeded')) for e in final['entries']),
        in_process_exact=sum(c['filtered_exact'] and c['unfiltered_exact'] for c in checks),
        in_process_calls=len(checks)))
    print('Exploratory run complete; no backend selected.', flush=True)


if __name__ == '__main__':
    main()
