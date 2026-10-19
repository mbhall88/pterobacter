# Install and use a database

Install the published database once:

```sh
ptero database install 2026-10-15
ptero database verify 2026-10-15
ptero --report-dir isolate-report isolate.fa
```

The database is published on [Figshare](https://doi.org/10.6084/m9.figshare.34640883.v1).
This build pins version `2026-10-15` as its default, including the archive hashes,
extracted-file inventories and component bindings. Default installation downloads
about 333 MB for reference sketches and whole-genome FASTAs. Classification works
offline after installation and does not download missing components.

To add optional historical MLST:

```sh
ptero database install 2026-10-15 --mlst
ptero --mlst --report-dir typed-report isolate.fa
```

Typing uses scheme records submitted on or before **31 December 2024**. It is not
a current PubMLST scheme. Installing it does not enable typing automatically;
request `--mlst` when classifying.

Use `ptero database list` to inspect versions and component presence. Select an
exact version with `ptero -D 2026-10-15 isolate.fa`; without `-D`, this build uses
the single published default pinned above. It never selects the newest directory
or substitutes another version. Downloads use the file URLs pinned for Figshare
record version 1, and every component must match its expected hashes.

On HPC systems, install to supported local storage using `--database-dir DIR` and
pass that option for classification too. Some shared filesystems, including a
tested GPFS filesystem, cannot support atomic activation and are refused.

## A working local example

This source-checkout example generates archives from the existing synthetic native
rescue and historical typing fixtures. It exercises provisioning and classification;
its invented organisms and MLST alleles are software test data.

```sh
cargo build --locked --bins
# Choose a new directory on local storage. The generator refuses an existing path.
demo="$(mktemp -d)/example"
cargo run --locked --example local_database -- "$demo"
# This locally generated catalogue has a locally generated pin. For real data,
# obtain this pin through the trusted handoff, independently of the download.
pin="$(cat "$demo/catalogue.sha256")"
ptero_cmd=target/debug/ptero
"$ptero_cmd" --catalogue "$demo/catalogue.json" --catalogue-sha256 "$pin" \
  --database-dir "$demo/store" database install fixture-1 \
  --from-directory "$demo/archives"
"$ptero_cmd" --catalogue "$demo/catalogue.json" --catalogue-sha256 "$pin" \
  --database-dir "$demo/store" -D fixture-1 "$demo/fixed-query.fa" > "$demo/report.json"
# Add typing later without replacing the reference or FASTAs.
"$ptero_cmd" --catalogue "$demo/catalogue.json" --catalogue-sha256 "$pin" \
  --database-dir "$demo/store" database install fixture-1 \
  --from-directory "$demo/archives" --mlst
"$ptero_cmd" --catalogue "$demo/catalogue.json" --catalogue-sha256 "$pin" \
  --database-dir "$demo/store" database verify fixture-1
"$ptero_cmd" --catalogue "$demo/catalogue.json" --catalogue-sha256 "$pin" \
  --database-dir "$demo/store" -D fixture-1 --mlst "$demo/fixed-query.fa" \
  > "$demo/typed-report.json"
```

The catalogue override always requires `-C/--catalogue` and
`-H/--catalogue-sha256` together. Local archive installation uses
`-q/--from-directory`, or repeated `-f/--from-file` paths whose filenames match the
catalogue. Neither local mode uses the network. A published catalogue entry may
supply an HTTPS download URL; certificate verification and HTTPS-only redirects
are enforced, with connection, response, body and total time limits. Archive size,
SHA-256 and decoded length remain mandatory. Candidates have no URLs or default.
Withdrawn entries require explicit selection, emit a warning and install only from
local archives. Retrieval alone is never a publication or scientific acceptance.

## Components and paths

Default installation includes `reference` and its bound `primary-fasta` component.
`--no-anim` installs only the reference. Optional typing is installed only with
`--mlst`, and requested during classification with `-l/--mlst`. Every selected
component is fully verified during reuse and classification. A missing required
component stops classification; missing or invalid requested typing produces a
failed MLST record and exit 6 while preserving a completed taxonomic result.

Linux defaults are `$XDG_DATA_HOME/pterobacter/databases` and
`$XDG_STATE_HOME/pterobacter/evidence`, falling back to
`$HOME/.local/share` and `$HOME/.local/state`. macOS uses
`$HOME/Library/Application Support/pterobacter` for both application roots.
`-B/--database-dir` overrides the database root. `-e/--anim-output` overrides
evidence placement. Otherwise each rescue-mode run reserves a unique, retained
state directory and reports it in `data_provenance.anim_evidence_directory`.
Missing directories are created privately; existing ancestor permissions stay as
configured. `--version` reads neither catalogue nor user directories.

An install holds one exclusive lock per database/version. Components are verified
in private staging directories on the destination filesystem and activated with
atomic no-replace rename. Installed bytes are immutable: corruption fails without
replacement. A later component failure can leave earlier verified components
installed; the requested set is reported ready only after all succeed. A failure
after activation explicitly says the component was activated. Retry fully verifies
it before reconstructing a missing receipt. Receipts live outside scientific
directories and never authorize reuse. Interrupted locks/stages remain for owner
inspection; cleanup removes only the current attempt's temporary files.

Linux uses `renameat2(RENAME_NOREPLACE)` and macOS uses
`renamex_np(RENAME_EXCL)`. Supported local storage is required for installation.
A tested shared GPFS filesystem rejected this operation; the installer refuses
unsupported filesystems safely. Select local storage with `--database-dir`, or
classify from verified explicit packages on shared storage. General HPC filesystem
installation support is not claimed.

## Explicit packages and provenance

`--reference-package DIR --expected-sha256 HEX` continues to accept development
and public package flavours and conflicts with `--database`. Explicit ANIm and
MLST path/digest pairs override only their own components and still pass full
verification. Explicit MLST is itself an opt-in. An explicit reference never gains
an unrelated default FASTA component: any supplied FASTAs must bind to its exact
manifest. No default catalogue is inferred from an installation receipt.

JSON reports identify `operator_pin`, `operator_pinned_catalogue` or
`embedded_catalogue` as the reference trust source. Selected catalogue citation
metadata appears in JSON and database inspection; ordinary `--version` remains a
tool version. Public flavour metadata is an export profile, not evidence of
publication, redistribution rights or independent scientific validation.

Historical MLST keeps its 31 December 2024 submission cutoff and limitation.
A 2026 retrieval date does not establish newer coverage. The separately versioned
[current/custom contract](contracts/MLST-PACKAGE-v2.md) supports compatible local
seven-locus schemes with explicit provenance and rights. Current PubMLST
provisioning remains unavailable pending source access and redistribution review;
this interface does not fetch a new scheme.
