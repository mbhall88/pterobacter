#!/usr/bin/env python3
"""Saved-output diagnostic: pass the comparator's and route D's results through the real, unchanged Enterobacter MLST importer.

    importer_diagnostic.py IMPORTER_SRC PACKAGE_DIR FIXTURE_DIR COMPARATOR_DIR CANDIDATE_DIR MLST_ENV OUT_TSV [--work DIR]

IMPORTER_SRC is the `src` directory of the Enterobacter repository (the package `enterobacter_classifier`); it is imported, never
modified. PACKAGE_DIR holds manifest.json and pubmlst/ecloacae/. COMPARATOR_DIR / CANDIDATE_DIR are saved run directories.

What this is and is not:
* No alignment is run. The comparator side uses mlst's own saved `mlst.json` record. The importer's input contract wants a
  prepared (never executed) command with executable and BLAST-index hashes, which are built here from the pinned environment and
  the comparator's saved BLAST database. Fixture paths and the manifest hash are reconstructed from the saved fixtures; this is
  a reconstructed receipt, not a historical execution receipt.
* Route D is not mlst. Its record has the same five fields, and the importer is called with the SAME prepared command so that the
  importer's input contract is satisfied. That command's `execution` block therefore names the comparator's executables; it is
  dropped before comparison and the route D importer outputs are diagnostic values, never evidence that the new engine ran mlst.
* Compared between the two sides (everything except `execution`): state, sequence_type, per-locus tokens, sequence-verified exact
  locations, profile-compatible candidate ST, copy-diagnostic reasons and the cutoff/limitation/provenance fields. The importer
  re-reads each fixture and checks every exact location against the allele sequence (or its reverse complement).
* compare.py's own derive() is also checked against the importer's state and candidate ST.
"""
import argparse
import csv
import hashlib
import json
import sys
from pathlib import Path

LOCI = ["dnaA", "fusA", "gyrB", "leuS", "pyrG", "rplB", "rpoB"]
DB_SUFFIXES = ("", ".ndb", ".nhr", ".nin", ".njs", ".nog", ".nos", ".not", ".nsq", ".ntf", ".nto")


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def rows_for(query_id, query_sha, locations):
    seen, rows = set(), []
    for loc in locations:
        key = (loc["locus"], str(loc["allele"]), loc["contig"], int(loc["start"]), int(loc["end"]))
        if key in seen:
            continue
        seen.add(key)
        rows.append(dict(accession=query_id, source_fasta_sha256=query_sha, locus=key[0], allele=key[1], contig=key[2], start=key[3], end=key[4]))
    return rows


def comparator_exact(parsed):
    return [h for h in parsed["hits"] if h["nident"] / h["hlen"] >= 0.5 and h["alen"] == h["hlen"] == h["nident"]]


def main():
    ap = argparse.ArgumentParser()
    for n in ("importer_src", "package", "fixtures", "comparator", "candidate", "mlst_env", "out"):
        ap.add_argument(n)
    ap.add_argument("--work", default=None)
    a = ap.parse_args()
    sys.path.insert(0, a.importer_src)
    from enterobacter_classifier import mlst as imp  # noqa: E402

    package, fixtures, comp, cand = Path(a.package), Path(a.fixtures), Path(a.comparator), Path(a.candidate)
    manifest_path = package / "manifest.json"
    ctx = imp.MlstContext(manifest_path, expected_sha256=sha256(manifest_path))
    bin_dir = Path(a.mlst_env) / "bin"
    db_prefix = comp / "blastdb" / "mlst.fa"
    runtime = {"executables": {"mlst": {"path": str(bin_dir / "mlst"), "sha256": sha256(bin_dir / "mlst"), "version": "2.32.2"},
                               "blastn": {"path": str(bin_dir / "blastn"), "sha256": sha256(bin_dir / "blastn"), "version": "2.16.0+"},
                               "any2fasta": {"path": str(bin_dir / "any2fasta"), "sha256": sha256(bin_dir / "any2fasta")}},
               "database": {"prefix": str(db_prefix), "sha256": {str(db_prefix) + s: sha256(str(db_prefix) + s) for s in DB_SUFFIXES}}}
    work = Path(a.work) if a.work else Path(a.out).parent / "importer-work"
    work.mkdir(parents=True, exist_ok=True)
    manifest = json.loads((fixtures / "manifest.json").read_text())

    def run_side(name, record, locations, command):
        try:
            out = ctx.interpret(record, command=command, search_exit_code=0, exact_locations=locations)
            return dict(ok=True, out=out)
        except ValueError as e:
            return dict(ok=False, error=str(e))

    def comparable(out):
        return {k: v for k, v in out.items() if k != "execution"}

    rows = []
    for name, meta in manifest["fixtures"].items():
        fa = (fixtures / meta["file"]).resolve()
        mj = json.loads((comp / name / "mlst.json").read_text())[0]
        record_c = dict(mj, filename=str(fa))
        qid, qsha = record_c["id"], sha256(fa)
        try:
            command = ctx.build_command(runtime=runtime, query_path=fa, query_id=qid, output_path=work / f"{name}.json", threads=1,
                                        min_identity_percent=95, min_coverage_percent=50)
        except ValueError as e:
            rows.append(dict(fixture=name, comparator_import=f"REJECTED at command build: {e}", route_d_import="not attempted"))
            continue
        parsed = json.loads((comp / name / "parsed.json").read_text())
        c = run_side("comparator", record_c, rows_for(qid, qsha, comparator_exact(parsed)), command)
        res = json.loads((cand / name / "result.json").read_text())["call"]
        record_d = dict(id=qid, filename=str(fa), scheme="ecloacae", sequence_type=res["ST"], alleles=dict(zip(LOCI, res["tokens"])))
        d = run_side("route_d", record_d, rows_for(qid, qsha, res["exact_locations"]), command)
        row = dict(fixture=name, comparator_import="ok" if c["ok"] else "REJECTED: " + c["error"],
                   route_d_import="ok" if d["ok"] else "REJECTED: " + d["error"])
        if c["ok"]:
            o = c["out"]
            row.update(importer_state=o["state"], importer_ST=o["sequence_type"] or "", importer_candidate_ST=o["profile_compatible_candidate_ST"] or "",
                       copy_reasons=",".join(o["copy_diagnostic_reasons"]), verified_exact_locations=sum(len(l["sequence_verified_exact_locations"]) for l in o["loci"]),
                       cutoff=o["submission_cutoff"], limitation_text_ok=o["limitation"] == imp.LIMITATION)
        if c["ok"] and d["ok"]:
            same = comparable(c["out"]) == comparable(d["out"])
            row["importer_outputs_equal"] = same
            if not same:
                row["differences"] = ",".join(k for k in comparable(c["out"]) if comparable(c["out"])[k] != comparable(d["out"]).get(k))
        elif c["ok"] != d["ok"] or (not c["ok"] and c["error"] != d["error"]):
            row["importer_outputs_equal"] = False
        else:
            row["importer_outputs_equal"] = True  # both sides rejected the input for the same reason
        rows.append(row)
        (work / f"{name}.comparator.json").write_text(json.dumps(c, indent=1, sort_keys=True))
        (work / f"{name}.route_d.json").write_text(json.dumps(dict(d, note="diagnostic: command/execution block is the comparator's"), indent=1, sort_keys=True))

    # does compare.py's own derive() agree with the importer?
    sys.path.insert(0, str(Path(__file__).parent))
    import compare
    alleles, profiles = compare.load_package(str(package / "pubmlst" / "ecloacae"))
    for row in rows:
        if row.get("importer_state") is None:
            continue
        parsed = json.loads((comp / row["fixture"] / "parsed.json").read_text())
        exact = [(h["locus"], h["allele"], h["contig"], h["start"], h["end"]) for h in comparator_exact(parsed)]
        state, cstj = compare.derive(parsed["tokens"], exact, alleles, profiles)
        row["harness_derive_agrees"] = (state == row["importer_state"]) and ((cstj or "") == row["importer_candidate_ST"])

    cols = ["fixture", "comparator_import", "route_d_import", "importer_state", "importer_ST", "importer_candidate_ST", "copy_reasons", "verified_exact_locations",
            "importer_outputs_equal", "differences", "harness_derive_agrees", "cutoff", "limitation_text_ok"]
    with open(a.out, "w", newline="") as fh:
        w = csv.DictWriter(fh, cols, delimiter="\t", extrasaction="ignore", restval="")
        w.writeheader()
        w.writerows(rows)
    ok = [r for r in rows if r["comparator_import"] == "ok"]
    print(f"{len(rows)} fixtures; importer accepted comparator output for {len(ok)}; "
          f"outputs equal (excl. execution block) for {sum(r.get('importer_outputs_equal') is True for r in rows)}; "
          f"harness derive() agrees with importer for {sum(r.get('harness_derive_agrees') is True for r in ok)}/{len(ok)}")
    for r in rows:
        if r["comparator_import"] != "ok" or r.get("importer_outputs_equal") is not True:
            print(" ", r["fixture"], r["comparator_import"], "|", r["route_d_import"], "|", r.get("differences", ""))


if __name__ == "__main__":
    main()
