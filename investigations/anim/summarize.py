"""Publish synthetic observations only; remove raw research argv/path receipts."""
import argparse
import csv
import json
from pathlib import Path

METRICS = ['status', 'D', 'E', 'reference_covered', 'query_covered', 'reference_length', 'query_length']
GATES = ['identity_gt96', 'reference_af_ge80', 'query_af_ge80']


def load_pairs(root):
    return [json.loads(p.read_text()) for p in sorted(root.glob('*/*/result.json'))]


def table(path, rows):
    with path.open('w', newline='') as out:
        writer = csv.DictWriter(out, fieldnames=list(rows[0]), delimiter='\t')
        writer.writeheader()
        writer.writerows(rows)


def summarize(first, fileapi, memorydiag, output):
    output.mkdir(parents=True, exist_ok=False)
    records = [r for r in load_pairs(first / 'matrix') if r['candidate'] in ('comparator', 'embedded-mummer3')]
    records += [r for r in load_pairs(fileapi / 'matrix') if r['candidate'].startswith('asm')]
    if len(records) != 100:
        raise ValueError('expected 20 pairs for five measured profiles')
    base = {r['fixture']: r for r in records if r['candidate'] == 'comparator'}
    memory = {r['fixture']: r for r in load_pairs(memorydiag / 'matrix') if r['candidate'] == 'embedded-mummer3'}
    observations, differences = [], []
    summary = {}
    for candidate in ['comparator', 'embedded-mummer3', 'asm5', 'asm10', 'asm20']:
        rows = [r for r in records if r['candidate'] == candidate]
        exact = gate_changes = delta_matches = 0
        for row in rows:
            reference = base[row['fixture']]
            exact += all(all(row[o]['metrics'][k] == reference[o]['metrics'][k] for k in METRICS) for o in ['QR', 'RQ'])
            gate_changes += any(row[o]['metrics'][k] != reference[o]['metrics'][k] for o in ['QR', 'RQ'] for k in GATES)
            delta_matches += all(row[o].get('normalized_delta_sha256') == reference[o].get('normalized_delta_sha256') for o in ['QR', 'RQ']) if candidate == 'embedded-mummer3' else 0
            for orientation in ['QR', 'RQ']:
                metric = row[orientation]['metrics']
                # RQ native reference is biological Q; map coverage back explicitly.
                biological_query_af = metric['query_af'] if orientation == 'QR' else metric['reference_af']
                biological_reference_af = metric['reference_af'] if orientation == 'QR' else metric['query_af']
                diagnostic_peak = memory[row['fixture']][orientation]['sampled_tree_peak_kib'] if candidate == 'embedded-mummer3' else row[orientation].get('sampled_tree_peak_kib')
                observations.append(dict(fixture=row['fixture'], stratum=row['stratum'], candidate=candidate, orientation=orientation,
                                         **metric, biological_query_af=biological_query_af, biological_reference_af=biological_reference_af,
                                         wall_seconds=row[orientation]['wall_seconds'], cpu_seconds=row[orientation]['cpu_seconds'],
                                         gnu_peak_kib=row[orientation]['peak_kib'] or None,
                                         sampled_tree_peak_kib=diagnostic_peak))
                if candidate != 'comparator':
                    cmp = reference[orientation]['metrics']
                    diff = dict(fixture=row['fixture'], stratum=row['stratum'], candidate=candidate, orientation=orientation,
                                comparator_identity=cmp['identity'], candidate_identity=metric['identity'],
                                identity_difference_pp=None if metric['identity'] is None or cmp['identity'] is None else metric['identity'] - cmp['identity'],
                                biological_query_af_difference_pp=biological_query_af - (cmp['query_af'] if orientation == 'QR' else cmp['reference_af']),
                                biological_reference_af_difference_pp=biological_reference_af - (cmp['reference_af'] if orientation == 'QR' else cmp['query_af']),
                                exact_integer_equal=all(metric[k] == cmp[k] for k in METRICS),
                                identity_gate_changed=metric['identity_gt96'] != cmp['identity_gt96'],
                                native_reference_af_gate_changed=metric['reference_af_ge80'] != cmp['reference_af_ge80'],
                                native_query_af_gate_changed=metric['query_af_ge80'] != cmp['query_af_ge80'])
                    differences.append(diff)
        summary[candidate] = dict(pairs=len(rows), exact_integer_pairs=exact, pairs_with_gate_changes=gate_changes,
                                  normalized_delta_equal_pairs=delta_matches if candidate == 'embedded-mummer3' else None,
                                  errors=sum(r[o]['metrics']['status'] == 'error' for r in rows for o in ['QR', 'RQ']),
                                  no_alignment_orientations=sum(r[o]['metrics']['status'] == 'no_alignment' for r in rows for o in ['QR', 'RQ']))
    panels = []
    for root, candidates in [(first, ['comparator', 'embedded-mummer3']), (fileapi, ['asm5', 'asm10', 'asm20'])]:
        for candidate in candidates:
            panels.append(json.loads((root / 'panel' / candidate / 'summary.json').read_text()))
    panel_records = {root: load_pairs(root / 'panel') for root in {first, fileapi}}
    baseline_panel = {r['fixture']: r for r in panel_records[first] if r['candidate'] == 'comparator'}
    for row in panels:
        root = first if row['candidate'] in ['comparator', 'embedded-mummer3'] else fileapi
        pairs = [r for r in panel_records[root] if r['candidate'] == row['candidate']]
        row['exact_integer_pairs'] = sum(all(all(r[o]['metrics'][k] == baseline_panel[r['fixture']][o]['metrics'][k] for k in METRICS) for o in ['QR', 'RQ']) for r in pairs)
        if row['candidate'] == 'embedded-mummer3':
            row['normalized_delta_equal_pairs'] = sum(all(r[o]['normalized_delta_sha256'] == baseline_panel[r['fixture']][o]['normalized_delta_sha256'] for o in ['QR', 'RQ']) for r in pairs)
    table(output / 'observations.tsv', observations)
    table(output / 'differences.tsv', differences)
    table(output / 'rescue-band-differences.tsv', [r for r in differences if r['stratum'] == 'rescue-band'])
    table(output / 'af-boundary-differences.tsv', [r for r in differences if r['stratum'] == 'AF-boundary'])
    (output / 'summary.json').write_text(json.dumps(dict(matrix=summary, panel=panels), indent=2) + '\n')
    print(json.dumps(summary, indent=2))

if __name__ == '__main__':
    p = argparse.ArgumentParser()
    for name in ['first', 'fileapi', 'memorydiag', 'output']:
        p.add_argument('--' + name, required=True, type=Path)
    a = p.parse_args()
    summarize(a.first, a.fileapi, a.memorydiag, a.output)
