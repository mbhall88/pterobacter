"""Development P1 metric reader. Never classifies a biological query."""
from collections import defaultdict
import re


def union_bases(intervals):
    covered = 0
    end = 0
    for start, stop in sorted((min(a, b), max(a, b)) for a, b in intervals):
        if start < 1:
            raise ValueError('nonpositive coordinate')
        covered += max(0, stop - max(start, end + 1) + 1)
        end = max(end, stop)
    return covered


def metric_record(d, e, ref_intervals, query_intervals, refs, queries, blocks):
    if d < 0 or e < 0 or 2 * e > d or (blocks == 0) != (d == 0):
        raise ValueError('impossible span/error record')
    def coverage(intervals, lengths):
        total = 0
        for key, spans in intervals.items():
            if key not in lengths or any(max(a, b) > lengths[key] for a, b in spans):
                raise ValueError('unknown record or coordinate out of bounds')
            total += union_bases(spans)
        return total
    rc, qc = coverage(ref_intervals, refs), coverage(query_intervals, queries)
    rl, ql = sum(refs.values()), sum(queries.values())
    if min(rl, ql) <= 0:
        raise ValueError('empty whole input')
    return dict(status='aligned' if blocks else 'no_alignment', D=d, E=e, blocks=blocks,
                reference_covered=rc, query_covered=qc, reference_length=rl, query_length=ql,
                identity=100 * (d - 2 * e) / d if d else None,
                reference_af=100 * rc / rl, query_af=100 * qc / ql,
                identity_gt96=bool(d and 100 * (d - 2 * e) > 96 * d),
                reference_af_ge80=100 * rc >= 80 * rl,
                query_af_ge80=100 * qc >= 80 * ql)


def delta_metrics(path, refs, queries):
    lines = path.read_text().splitlines()
    if len(lines) < 2 or lines[1] != 'NUCMER' or len(lines[0].split()) != 2:
        raise ValueError('invalid delta header')
    d = e = blocks = 0
    ri, qi = defaultdict(list), defaultdict(list)
    context = None
    offsets = False
    for line in lines[2:]:
        if line.startswith('>'):
            if offsets:
                raise ValueError('unterminated delta block')
            r, q, rl, ql = line[1:].split()
            if refs.get(r) != int(rl) or queries.get(q) != int(ql):
                raise ValueError('delta record length/ID mismatch')
            context = r, q
        elif offsets:
            if len(line.split()) != 1:
                raise ValueError('invalid delta offset')
            if int(line) == 0:
                offsets = False
        else:
            fields = list(map(int, line.split()))
            if context is None or len(fields) != 7:
                raise ValueError('invalid delta block')
            rs, re_, qs, qe, errors, _, _ = fields
            ri[context[0]].append((rs, re_))
            qi[context[1]].append((qs, qe))
            span = abs(rs - re_) + abs(qs - qe) + 2
            if errors < 0 or 2 * errors > span:
                raise ValueError('invalid delta errors')
            d += span
            e += errors
            blocks += 1
            offsets = True
    if offsets:
        raise ValueError('truncated delta')
    return metric_record(d, e, ri, qi, refs, queries, blocks)


def paf_metrics(path, refs, queries):
    d = e = blocks = 0
    ri, qi = defaultdict(list), defaultdict(list)
    for line in path.read_text().splitlines():
        fields = line.split('\t')
        if len(fields) < 14:
            raise ValueError('incomplete base-level PAF')
        q, qlen, qs, qe, strand, r, rlen, rs, re_ = fields[:9]
        qlen, qs, qe, rlen, rs, re_ = map(int, (qlen, qs, qe, rlen, rs, re_))
        if (refs.get(r) != rlen or queries.get(q) != qlen or strand not in ('+', '-')
                or not 0 <= qs < qe <= qlen or not 0 <= rs < re_ <= rlen):
            raise ValueError('PAF length/coordinate/ID mismatch')
        tags = {tag.split(':', 2)[0]: tag.split(':', 2)[1:] for tag in fields[12:]}
        if tags.get('NM', [''])[0] != 'i' or tags.get('cg', [''])[0] != 'Z':
            raise ValueError('missing alignment NM/CIGAR')
        errors = int(tags['NM'][1])
        cigar = tags['cg'][1]
        operations = re.findall(r'(\d+)([MID=XS])', cigar)
        if ''.join(n + op for n, op in operations) != cigar or any(int(n) <= 0 for n, _ in operations):
            raise ValueError('unsupported CIGAR')
        # Older binding cigar_str adds SAM soft clips even when cigar_clipping=false.
        # Accept only correct terminal clips; aligned spans always exclude them.
        for index, (length, op) in enumerate(operations):
            if op == 'S':
                if index not in (0, len(operations) - 1):
                    raise ValueError('internal soft clip')
                expected = (qs if strand == '+' else qlen - qe) if index == 0 else (qlen - qe if strand == '+' else qs)
                if int(length) != expected:
                    raise ValueError('incorrect terminal soft clip')
        reference_span = sum(int(n) for n, op in operations if op in 'MD=X')
        query_span = sum(int(n) for n, op in operations if op in 'MI=X')
        gaps = sum(int(n) for n, op in operations if op in 'ID')
        mismatches_max = sum(int(n) for n, op in operations if op in 'MX')
        if (reference_span != re_ - rs or query_span != qe - qs
                or not gaps <= errors <= gaps + mismatches_max):
            raise ValueError('CIGAR/span/NM mismatch')
        ri[r].append((rs + 1, re_))
        qi[q].append((qs + 1, qe))
        d += reference_span + query_span
        e += errors
        blocks += 1
    return metric_record(d, e, ri, qi, refs, queries, blocks)
