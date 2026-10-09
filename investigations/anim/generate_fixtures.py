"""Generate the preregistered synthetic fixture matrix, never read research genomes."""
import argparse
import hashlib
import json
import pathlib
import random

SEED = 20261009


def dna(length, seed):
    return random.Random(seed).randbytes(length).translate(bytes.maketrans(bytes(range(256)), bytes(b'ACGT'[i % 4] for i in range(256)))).decode()


def mutate(sequence, rate, seed):
    generator = random.Random(seed)
    result = bytearray(sequence.encode())
    for index in generator.sample(range(len(result)), round(len(result) * rate)):
        original = result[index]
        result[index] = generator.choice([b for b in b'ACGT' if b != original])
    return result.decode()


def reverse_complement(sequence):
    return sequence.translate(str.maketrans('ACGT', 'TGCA'))[::-1]


def write_fasta(path, records):
    with path.open('w') as stream:
        for name, sequence in records:
            stream.write('>' + name + '\n')
            for offset in range(0, len(sequence), 80):
                stream.write(sequence[offset:offset + 80] + '\n')


def generate(root):
    root.mkdir(parents=True, exist_ok=False)
    base = dna(120000, SEED)
    cases = []
    def add(name, reference, query, stratum='other', valid=True):
        ref, qry = root / (name + '.r.fa'), root / (name + '.q.fa')
        write_fasta(ref, reference)
        write_fasta(qry, query)
        cases.append(dict(id=name, reference=ref.name, query=qry.name, stratum=stratum, valid=valid))
    add('identical', [('r', base)], [('q', base)])
    add('reverse_complement', [('r', base)], [('q', reverse_complement(base))])
    for index, rate in enumerate([.04, .0401, .045, .05, .055, .06]):
        name = 'sub_' + str(round(10000 * rate))
        add(name, [('r', base)], [('q', mutate(base, rate, SEED + 100 + index))], 'rescue-band' if rate <= .055 else 'outside-band')
    for index, fraction in enumerate([.799, .8, .801]):
        length = round(len(base) * fraction)
        add('af_' + str(round(fraction * 1000)), [('r', base)], [('q', base[:length] + dna(len(base) - length, SEED + 200 + index))], 'AF-boundary')
    add('indel_tracts', [('r', base)], [('q', base[:30000] + dna(350, SEED + 300) + base[30000:70000] + base[70500:])])
    add('inversion', [('r', base)], [('q', base[:30000] + reverse_complement(base[30000:80000]) + base[80000:])])
    chunks = [base[x:x + 30000] for x in range(0, len(base), 30000)]
    add('permutation', [('r', base)], [('q', chunks[2] + chunks[0] + chunks[3] + chunks[1])])
    repeated = base[:40000] + base[40000:60000] * 3 + base[60000:]
    add('repeat_reference', [('r', repeated)], [('q', base)])
    add('repeat_query', [('r', base)], [('q', repeated)])
    add('fragmented', [('r', base)], [('q' + str(i), base[i * 10000:(i + 1) * 10000]) for i in range(12)])
    ambiguous = bytearray(base.encode())
    for position in random.Random(SEED + 400).sample(range(len(base)), 2400):
        ambiguous[position] = ord('N')
    add('ambiguous', [('r', base)], [('q', ambiguous.decode())])
    add('no_alignment', [('r', base)], [('q', dna(len(base), SEED + 500))])
    add('invalid_alphabet', [('r', base)], [('q', base[:100] + 'Z' + base[101:])], valid=False)
    add('duplicate_ids', [('r', base)], [('q', base[:60000]), ('q', base[60000:])], valid=False)
    add('empty_fasta', [('r', base)], [], valid=False)
    cases.append(dict(id='missing_file', reference='identical.r.fa', query='missing.fa', stratum='other', valid=False))
    genome = dna(5000000, SEED + 600)
    add('genome_5mb', [('r', genome)], [('q', mutate(genome, .045, SEED + 601))], 'genome-cost')
    panel_base = dna(500000, SEED + 700)
    write_fasta(root / 'panel-query.fa', [('query', panel_base)])
    panel = []
    for i in range(140):
        name = f'panel-{i:03}.fa'
        rate = .1 * i / 139
        write_fasta(root / name, [('ref', mutate(panel_base, rate, SEED + 800 + i))])
        panel.append(dict(id=f'panel-{i:03}', reference=name, query='panel-query.fa', stratum='panel-cost', valid=True, nominal_substitution_rate=rate))
    hashes = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in root.iterdir() if p.is_file()}
    manifest = dict(seed=SEED, cases=cases, panel=panel, sha256=hashes)
    (root / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print(f'{len(cases)} fixtures and {len(panel)} panel references; manifest saved')

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=pathlib.Path)
    generate(parser.parse_args().output)
