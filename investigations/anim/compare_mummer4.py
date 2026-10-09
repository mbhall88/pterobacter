"""Compare retained delta blocks and P1 records; never execute an aligner."""
import argparse
import hashlib
import json
import pathlib
from measure import fasta_lengths
from metrics import delta_metrics


INTEGER_FIELDS = ('status', 'D', 'E', 'reference_covered', 'query_covered', 'reference_length', 'query_length')
GATES = ('identity_gt96', 'reference_af_ge80', 'query_af_ge80')


def canonical_delta(path):
    """Sort complete blocks with their record context; never sort offsets."""
    lines = path.read_text().splitlines()
    if len(lines) < 2 or lines[1] != 'NUCMER':
        raise ValueError('invalid delta header')
    context = None
    block = None
    blocks = []
    for line in lines[2:]:
        if line.startswith('>'):
            if block is not None:
                raise ValueError('unterminated block')
            context = line
        elif block is None:
            if context is None or len(line.split()) != 7:
                raise ValueError('invalid block header')
            block = [context, line]
        else:
            if len(line.split()) != 1:
                raise ValueError('invalid delta offset')
            int(line)
            block.append(line)
            if line == '0':
                blocks.append(tuple(block))
                block = None
    if block is not None:
        raise ValueError('truncated delta')
    return sorted(blocks)


def canonical_sha(path):
    return hashlib.sha256(json.dumps(canonical_delta(path), separators=(',', ':')).encode()).hexdigest()


def compare(old, new, fixtures, output):
    rows = []
    for p in sorted(new.glob('*/result.json')):
        pair = json.loads(p.read_text())
        base = old / pair['fixture']
        prior = json.loads((base / 'result.json').read_text())
        for direction in ('QR', 'RQ'):
            a, b = prior[direction]['metrics'], pair[direction]['metrics']
            valid = all(m['status'] in ('aligned', 'no_alignment') for m in (a, b))
            row = dict(fixture=pair['fixture'], stratum=pair['stratum'], orientation=direction,
                       comparator=a, candidate=b,
                       exact_integers=all(a.get(k) == b.get(k) for k in INTEGER_FIELDS) if valid else None,
                       gate_changes={k: a[k] != b[k] for k in GATES} if valid else None,
                       continuous_changes={k: None if a[k] is None or b[k] is None else b[k]-a[k] for k in ('identity','reference_af','query_af')} if valid else None)
            for stage, name in [('unfiltered','native.delta'),('filtered','native.filter')]:
                left, right = base/direction/name, p.parent/direction/name
                if left.exists() and right.exists():
                    # Check against the same bound reference/query lengths first.
                    case = next(c for c in json.loads((fixtures/'manifest.json').read_text())['cases'] + json.loads((fixtures/'manifest.json').read_text())['panel'] if c['id']==pair['fixture'])
                    ref,qry = [fixtures/case[k] for k in ('reference','query')]
                    if direction=='RQ':ref,qry=qry,ref
                    lengths=fasta_lengths(ref),fasta_lengths(qry)
                    for raw in (left,right):delta_metrics(raw,*lengths)
                    row[stage] = dict(comparator_raw_sha256=hashlib.sha256(left.read_bytes()).hexdigest(),candidate_raw_sha256=hashlib.sha256(right.read_bytes()).hexdigest(),
                                      normalized_order_equal=left.read_text().splitlines()[1:]==right.read_text().splitlines()[1:],
                                      canonical_equal=canonical_delta(left)==canonical_delta(right),
                                      comparator_canonical_sha256=canonical_sha(left),candidate_canonical_sha256=canonical_sha(right))
            rows.append(row)
    output.parent.mkdir(parents=True,exist_ok=True)
    output.write_text(json.dumps(dict(orientations=rows,
        exact_pairs=sum(all(r['exact_integers'] is True for r in rows if r['fixture']==fixture) for fixture in {r['fixture'] for r in rows}),
        pairs=len(rows)//2,
        gate_changed_pairs=len({r['fixture'] for r in rows if r['gate_changes'] and any(r['gate_changes'].values())}),
        failed_orientations=sum(r['exact_integers'] is None for r in rows),
        filtered_canonical_equal_orientations=sum(r.get('filtered',{}).get('canonical_equal',False) for r in rows)),indent=2)+'\n')
    return rows


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    for name in ('old','new','fixtures','output'):parser.add_argument('--'+name,type=pathlib.Path,required=True)
    args=parser.parse_args()
    compare(args.old,args.new,args.fixtures,args.output)
