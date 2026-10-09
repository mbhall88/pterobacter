#!/usr/bin/env python3
"""Run the pinned mlst 2.32.2 + BLAST+ 2.16.0 comparator over the fixed fixtures.

    run_comparator.py PACKAGE_DIR FIXTURE_DIR OUT_DIR [fixture names...]
Writes OUT_DIR/<fixture>/{mlst.tsv,mlst.debug,mlst.json,record.json,parsed.json}.
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from common import MLST_ENV, LOCI, parse_comparator, sha256, timed


def main(package, fixtures, out, names):
    package, fixtures, out = Path(package), Path(fixtures), Path(out)
    out.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    env["PATH"] = f"{MLST_ENV / 'bin'}{os.pathsep}{env['PATH']}"
    env.pop("PERL5LIB", None)
    scheme_dir = package / "pubmlst" / "ecloacae"
    db_dir = out / "blastdb"
    fasta = db_dir / "mlst.fa"
    if not (db_dir / "mlst.fa.nsq").exists():
        db_dir.mkdir(exist_ok=True)
        with fasta.open("w") as fh:
            for loc in LOCI:
                for line in (scheme_dir / f"{loc}.tfa").read_text().splitlines():
                    fh.write(("&gt;" if False else ">ecloacae." + line[1:]) + "\n" if line.startswith(">") else line + "\n")
        rec = timed(["makeblastdb", "-hash_index", "-in", fasta, "-dbtype", "nucl", "-title", "PubMLST", "-parse_seqids"],
                    stdout=db_dir / "makeblastdb.stdout", stderr=db_dir / "makeblastdb.stderr", env=env)
        assert rec["exit_code"] == 0, rec
        (db_dir / "makeblastdb.record.json").write_text(json.dumps(rec, indent=1))
    manifest = json.loads((fixtures / "manifest.json").read_text())
    jobs = [(n, fixtures / manifest["fixtures"][n]["file"]) for n in (names or manifest["fixtures"])]
    if not names:
        jobs += [("invalid_" + p.stem, p) for p in sorted((fixtures / "invalid").iterdir())]
        jobs.append(("invalid_missing_file", fixtures / "invalid" / "does_not_exist.fna"))
    for name, fa in jobs:
        d = out / name
        if (d / "record.json").exists():
            continue
        d.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="mlstcmp-", dir="/tmp") as tmp:
            e = dict(env, TMPDIR=tmp)
            argv = ["mlst", "--scheme", "ecloacae", "--datadir", package / "pubmlst", "--blastdb", fasta,
                    "--minid", "95", "--mincov", "50", "--threads", "1", "--full", "--json", d / "mlst.json",
                    "--debug", "--nopath", fa]
            rec = timed(argv, stdout=d / "mlst.tsv", stderr=d / "mlst.debug", env=e)
        rec["input_sha256"] = sha256(fa) if fa.exists() else None
        rec["fixture"] = name
        (d / "record.json").write_text(json.dumps(rec, indent=1))
        if rec["exit_code"] == 0:
            parsed = parse_comparator(Path(d / "mlst.tsv").read_text().splitlines()[1], Path(d / "mlst.debug").read_text())
            (d / "parsed.json").write_text(json.dumps(parsed, indent=1))
        else:
            print(name, "exit", rec["exit_code"])


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:])
