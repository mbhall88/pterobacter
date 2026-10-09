#!/usr/bin/env python3
"""Build the fixed MLST fixture list from protocol.md (seed 20261009).

Synthetic assemblies: random background with package alleles embedded. Output is
written to a new directory and is not committed. Usage:
    make_fixtures.py PACKAGE_PUBMLST_DIR OUT_DIR
"""
import hashlib
import json
import random
import sys
from pathlib import Path

SEED = 20261009
LOCI = ["dnaA", "fusA", "gyrB", "leuS", "pyrG", "rplB", "rpoB"]
COMP = str.maketrans("ACGTacgt", "TGCAtgca")


def rc(s):
    return s.translate(COMP)[::-1]


def read_fasta(path):
    out, name = {}, None
    for line in Path(path).read_text().splitlines():
        if line.startswith(">"):
            name = line[1:].split()[0]
            out[name] = []
        elif name:
            out[name].append(line.strip())
    return {k: "".join(v) for k, v in out.items()}


def load(pkg):
    pkg = Path(pkg)
    alleles = {}
    for loc in LOCI:
        alleles[loc] = {k.rsplit("_", 1)[-1] if "_" in k else k.rsplit("-", 1)[-1]: v
                        for k, v in read_fasta(pkg / f"{loc}.tfa").items()}
    rows = [l.split("\t") for l in (pkg / "ecloacae.txt").read_text().splitlines()]
    profiles = {r[0]: r[1:8] for r in rows[1:]}
    return alleles, profiles


class Gen:
    def __init__(self, name):
        self.rng = random.Random(f"{SEED}/{name}")

    def bg(self, n, gc=0.55):
        r = self.rng
        return "".join(r.choice("GC") if r.random() < gc else r.choice("AT") for _ in range(n))

    def other_base(self, b):
        return self.rng.choice([x for x in "ACGT" if x != b.upper()])

    def subst(self, seq, positions):
        s = list(seq)
        for p in positions:
            s[p] = self.other_base(s[p])
        return "".join(s)

    def spread_subst(self, seq, n):
        L = len(seq)
        step = L / n
        pos = sorted({min(L - 1, int(step * i + step * self.rng.random())) for i in range(n)})
        return self.subst(seq, pos)


def write_fasta(path, contigs):
    with open(path, "w") as fh:
        for name, seq in contigs:
            fh.write(f">{name}\n")
            for i in range(0, len(seq), 80):
                fh.write(seq[i:i + 80] + "\n")


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main(pkg, out):
    alleles, profiles = load(pkg)
    out = Path(out)
    out.mkdir(parents=True, exist_ok=False)
    fixtures = {}  # name -> (description, contigs)

    def st_alleles(st):
        return {l: alleles[l][a] for l, a in zip(LOCI, profiles[st])}

    def one_contig(name, pieces, flank=1500, gap=2500, desc=""):
        """pieces: list of seq (allele-like) placed in order separated by random background."""
        g = Gen(name)
        s = g.bg(flank)
        for p in pieces:
            s += p + g.bg(gap)
        fixtures[name] = (desc, [("contig1", s)])

    def st_fixture(name, st, strands="f", desc=""):
        al = st_alleles(st)
        pieces = []
        for i, l in enumerate(LOCI):
            seq = al[l]
            if strands == "r" or (strands == "m" and i % 2):
                seq = rc(seq)
            pieces.append(seq)
        one_contig(name, pieces, desc=desc or f"ST{st}, strands={strands}")

    mid_st = sorted(profiles, key=int)[len(profiles) // 2]
    for st in ("1", "2480", "3295", mid_st):
        st_fixture(f"exact_st{st}", st)
    st_fixture("exact_st1_rev", "1", "r", "ST1 all reverse-complemented")
    st_fixture("exact_st1_mixed", "1", "m", "ST1 mixed strands")

    # exact seven alleles forming no recorded profile
    known = {tuple(v) for v in profiles.values()}
    base = profiles["1"]
    for alt in profiles.values():
        cand = [alt[0]] + base[1:]
        cand[1] = profiles["2"][1]
        if tuple(cand) not in known and cand[0] != base[0]:
            break
    pieces = [alleles[l][a] for l, a in zip(LOCI, cand)]
    one_contig("exact_unmatched_profile", pieces, desc="exact alleles, no recorded profile " + "/".join(cand))

    base_al = st_alleles("1")
    L0 = "dnaA"

    def with_locus(name, replacements, desc, drop=()):
        """ST1 assembly where listed loci are replaced (value: sequence or list of sequences) or dropped."""
        pieces = []
        for l in LOCI:
            if l in drop:
                continue
            v = replacements.get(l, base_al[l])
            pieces.extend(v if isinstance(v, list) else [v])
        one_contig(name, pieces, desc=desc)

    with_locus("missing_1", {}, "dnaA absent", drop=("dnaA",))
    with_locus("missing_3", {}, "dnaA, gyrB, rpoB absent", drop=("dnaA", "gyrB", "rpoB"))
    g = Gen("random_only")
    fixtures["random_only"] = ("no alleles at all", [(f"contig{i}", g.bg(20000)) for i in range(1, 4)])

    # inexact full length
    a = base_al[L0]
    L = len(a)
    g = Gen("inexact")
    with_locus("inexact_mid", {L0: g.subst(a, [L // 2])}, "one substitution mid-allele")
    for d in (0, 2, 9):
        with_locus(f"inexact_end{d + 1}", {L0: g.subst(a, [L - 1 - d])}, f"substitution {d} bases from the 3' end")
        with_locus(f"inexact_start{d + 1}", {L0: g.subst(a, [d])}, f"substitution {d} bases from the 5' end")
    n_hi, n_lo = int(L * 0.049), int(L * 0.051) + 1
    with_locus("inexact_id951", {L0: g.spread_subst(a, n_hi)}, f"{n_hi} spread substitutions of {L} (>=95.1% identity)")
    with_locus("inexact_id949", {L0: g.spread_subst(a, n_lo)}, f"{n_lo} spread substitutions of {L} (<=94.9% identity)")
    with_locus("inexact_five_loci", {l: g.subst(base_al[l], [len(base_al[l]) // 2]) for l in LOCI[:5]},
               "one mid substitution in five loci")

    # nearest-allele tie rules: two same-length alleles at Hamming distance 1
    pair = None
    byl = {}
    for aid, s in alleles[L0].items():
        byl.setdefault(len(s), []).append((aid, s))
    for ln, items in sorted(byl.items()):
        for i in range(len(items)):
            for j in range(i + 1, len(items)):
                d = [k for k, (x, y) in enumerate(zip(items[i][1], items[j][1])) if x != y]
                if len(d) == 1:
                    pair = (items[i], items[j], d[0])
                    break
            if pair:
                break
        if pair:
            break
    if pair:
        (ida, sa), (idb, sb), p = pair
        third = next(b for b in "ACGT" if b not in (sa[p], sb[p]))
        tied = sa[:p] + third + sa[p + 1:]
        with_locus("tie_two_alleles", {L0: tied}, f"variant equidistant (1 mismatch) from dnaA {ida} and {idb}")
        other = (p + 100) % len(sa)
        with_locus("tie_broken_by_one_snp", {L0: g.subst(sa, [other])},
                   f"dnaA {ida} plus a substitution away from the {ida}/{idb} difference site: tie remains")

    # partial alleles
    for pct in (70, 51, 49, 30):
        k = int(L * pct / 100)
        with_locus(f"partial_{pct}", {L0: a[:k]}, f"dnaA 5' {pct}% only")
    for n in (40, 60):
        with_locus(f"fragment_{n}", {L0: a[:n]}, f"dnaA {n}-base fragment (e-value floor)")

    # contig edges
    g = Gen("edges")
    pieces_rest = [base_al[l] for l in LOCI[1:]]
    def rest(name):
        gg = Gen(name)
        s = ""
        for p in pieces_rest:
            s += gg.bg(2500) + p
        return s + gg.bg(2500)
    fixtures["edge_exact_at_end"] = ("dnaA is the last bases of contig1", [("contig1", g.bg(1500) + a), ("contig2", rest("e1"))])
    fixtures["edge_exact_at_start"] = ("dnaA is the first bases of contig1", [("contig1", a + g.bg(1500)), ("contig2", rest("e2"))])
    cut = int(L * 0.3)
    fixtures["edge_trunc_30_start"] = ("dnaA missing first 30% (contig starts inside)", [("contig1", a[cut:] + g.bg(1500)), ("contig2", rest("e3"))])
    fixtures["edge_trunc_30_end"] = ("dnaA missing last 30% (contig ends inside)", [("contig1", g.bg(1500) + a[:L - cut]), ("contig2", rest("e4"))])
    half = L // 2
    fixtures["edge_split_two_contigs"] = ("dnaA cut in half across two contigs", [("contig1", g.bg(1500) + a[:half]), ("contig2", a[half:] + g.bg(1500)), ("contig3", rest("e5"))])
    fixtures["edge_one_contig_per_allele"] = ("each allele on its own contig", [(f"contig{i+1}", g.bg(800) + base_al[l] + g.bg(800)) for i, l in enumerate(LOCI)])

    # indels
    g = Gen("indel")
    m = L // 2
    with_locus("indel_ins1", {L0: a[:m] + g.other_base(a[m]) + a[m:]}, "1-base insertion mid-allele")
    with_locus("indel_del1", {L0: a[:m] + a[m + 1:]}, "1-base deletion mid-allele")
    with_locus("indel_ins3", {L0: a[:m] + "ACG" + a[m:]}, "3-base insertion mid-allele")
    with_locus("indel_ins1_end10", {L0: a[:L - 10] + g.other_base(a[L - 10]) + a[L - 10:]}, "1-base insertion 10 bases from the end")
    with_locus("indel_del1_30pct", {L0: a[:int(L * .3)] + a[int(L * .3) + 1:]}, "1-base deletion at 30% (both HSPs under/over 50%)")

    # duplicates
    other_id = next(i for i in alleles[L0] if alleles[L0][i] != a and i != "1")
    ob = alleles[L0][other_id]
    g = Gen("dups")
    with_locus("dup_same_twice", {L0: [a, a]}, "same dnaA allele at two locations")
    with_locus("dup_two_alleles", {L0: [a, ob]}, f"dnaA 1 and dnaA {other_id} both exact")
    with_locus("dup_exact_then_inexact", {L0: [a, g.subst(ob, [len(ob) // 2])]}, "exact allele then an inexact other allele")
    with_locus("dup_inexact_then_exact", {L0: [g.subst(ob, [len(ob) // 2]), a]}, "inexact other allele then exact allele")
    with_locus("dup_diverged_90", {L0: [a, g.spread_subst(a, int(L * .10))]}, "exact copy plus a ~90% identity copy")
    with_locus("dup_diverged_96", {L0: [a, g.spread_subst(a, int(L * .04))]}, "exact copy plus a ~96% identity copy")
    with_locus("dup_same_rev", {L0: [a, rc(a)]}, "same dnaA allele on both strands")

    # ambiguous bases
    g = Gen("amb")
    with_locus("amb_N1", {L0: a[:L // 2] + "N" + a[L // 2 + 1:]}, "one N inside dnaA")
    n_pos = sorted(g.rng.sample(range(L), int(L * .02)))
    with_locus("amb_N2pct", {L0: "".join("N" if i in set(n_pos) else c for i, c in enumerate(a))}, "2% N across dnaA")
    with_locus("amb_Nend", {L0: a[:-1] + "N"}, "N as the last allele base")
    with_locus("amb_iupac_R", {L0: a[:L // 2] + "R" + a[L // 2 + 1:]}, "IUPAC R inside dnaA")
    with_locus("lowercase", {L0: a.lower(), "fusA": base_al["fusA"].lower()}, "soft-masked lowercase alleles")

    # fragmentation
    g = Gen("frag12")
    full = "".join(g.bg(1500) + base_al[l] for l in LOCI) + g.bg(1500)
    cuts = sorted(g.rng.sample(range(1000, len(full) - 1000), 11))
    parts, prev = [], 0
    for c in cuts + [len(full)]:
        parts.append(full[prev:c])
        prev = c
    fixtures["frag_12_contigs"] = ("ST1 assembly cut at random points into 12 contigs (cuts may split alleles)",
                                   [(f"contig{i+1}", p) for i, p in enumerate(parts)])

    for name, (desc, contigs) in fixtures.items():
        write_fasta(out / f"{name}.fna", contigs)

    # invalid inputs
    inv = out / "invalid"
    inv.mkdir()
    (inv / "empty.fna").write_text("")
    (inv / "protein.faa").write_text(">p1\nMKVLAAGIVGLLLAQPSTEHRQWYNDF\n")
    s = fixtures["exact_st1"][1][0][1]
    write_fasta(inv / "duplicate_ids.fna", [("dup", s), ("dup", s[:5000])])
    (inv / "no_header.fna").write_text("ACGT" * 50 + "\n")

    manifest = {"seed": SEED, "package_dir": str(pkg),
                "fixtures": {n: {"description": d, "file": f"{n}.fna", "sha256": sha(out / f"{n}.fna"),
                                 "contigs": len(c), "bases": sum(len(x[1]) for x in c)} for n, (d, c) in fixtures.items()},
                "invalid": {p.name: sha(p) for p in sorted(inv.iterdir())}}

    # performance workloads
    perf = out / "perf"
    perf.mkdir()
    g = Gen("perf5mb")
    bg_len = 5_000_000
    chunks = [g.bg(1_000_000) for _ in range(5)]
    s = "".join(chunks)
    pos = sorted(g.rng.sample(range(100_000, bg_len - 100_000, 600_000), 7))
    s_list = []
    prev = 0
    for p, l in zip(pos, LOCI):
        s_list.append(s[prev:p] + base_al[l])
        prev = p
    s_list.append(s[prev:])
    genome = "".join(s_list)
    write_fasta(perf / "5mb_1contig.fna", [("chr", genome)])
    step = len(genome) // 200
    write_fasta(perf / "5mb_200contigs.fna", [(f"c{i}", genome[i * step:(i + 1) * step if i < 199 else None]) for i in range(200)])
    (perf / "panel").mkdir()
    for i in range(20):
        gg = Gen(f"panel{i}")
        al = dict(base_al)
        if i % 2:
            for l in gg.rng.sample(LOCI, 1 + i % 3):
                al[l] = gg.subst(al[l], [len(al[l]) // 3])
        bgs = gg.bg(5_000_000 - 20_000)
        cut = sorted(gg.rng.sample(range(100_000, len(bgs) - 100_000, 600_000), 7))
        pieces, prev = [], 0
        for c, l in zip(cut, LOCI):
            pieces.append(bgs[prev:c] + (al[l] if gg.rng.random() < .5 else rc(al[l])))
            prev = c
        pieces.append(bgs[prev:])
        write_fasta(perf / "panel" / f"panel_{i:02d}.fna", [("chr", "".join(pieces))])
    manifest["perf"] = {str(p.relative_to(perf)): sha(p) for p in sorted(perf.rglob("*.fna"))}
    (out / "manifest.json").write_text(json.dumps(manifest, indent=1) + "\n")
    print(len(fixtures), "fixtures;", len(manifest["perf"]), "perf files")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
