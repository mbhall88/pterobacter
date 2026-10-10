# Reference package fixture (`reference-v2/`)

A synthetic version 2 reference package for the authentication and binding tests. It carries no real reference data and is not the frozen package. `tests/common/package.rs` loads it into memory and damages one thing at a time.

The fixture copies the structure of the frozen package as well as its file layout. The research project's own package builder and exporter built it, and the conversion used for the frozen package then converted it. The research validator accepted every invariant the tests rely on before the tests used it.

## Identity

| Field | Value |
|---|---|
| Package id | `synthetic-enterobacter-reference` |
| Package version | `2026-10-09.test2` |
| SHA-256 of `package/portable.json` (version 2) | `d594ff403cf479ce677b39ff018cdceb3e5f08582610c6f519e927c16de9f797` |
| SHA-256 of `package/conversion/source-portable.json` (the version 1 manifest) | `0a48da33dc2adef2feefa592ef7a9ed9e15a184168231bb8ca826f2df80f3b28` |
| SHA-256 of `package/metadata/source-manifest.json` | `7ce3f86797b49b1e4735ee8739d33cbd574c3d96bfbf6321764ec78c4adee020` |
| SHA-256 of `package/metadata/export-validation.json` | `f0992ea29b84705dc897dd3a72bf1a766057beeb57fb0b6913fe2cb4b6c7e639` |
| SHA-256 of `package/licence/manifest.json` | `af0a36e88e315e6ae07e1f2b68618e1901ca339152cb1f98a8e5393c5846b657` |
| Licence disclaimer | `Synthetic test disclaimer. A real package carries the NCBI molecular data usage statement.` |

`tests/common/package.rs` pins the two manifest digests as `PINNED_MANIFEST_SHA256` and `CONVERTED_FROM_SHA256`. A test checks the committed files against them, so a silent change to the fixture fails the suite.

## What it exercises

The cases below occur in the frozen package. The earlier fixture had none of them.

- Overlapping components. `GCA_900000002.1` and `GCA_900000003.1` are both primary references and genomovar definition members. Each is one accession with two scoped references, `primary_whole:<accession>` and `genomovar_chromosome:<accession>`. The package has 9 scope records over 7 accessions.
- A cohort that is not the packaged references.
  - `GCA_900000005.1` is a primary reference outside the cohort.
  - `GCA_900000006.1` and `GCA_900000007.1` are cohort rows that are definition members only.
  - `GCA_900000030.1` is a cohort row with a represented concept and no packaged reference.
  - `GCA_900000031.1` names a species that has no reference.
  - `GCA_900000032.1` names only a context lineage.
  - `GCA_900000020.1` and `GCA_900000021.1` are development context records that are not packaged.
  - The five coverage categories are summarised as written.
- Concepts at every rank: three species, two subspecies, two context lineages and three genomovars with versioned definitions. One species has no reference.
- A type anchor that fails QC (`GCA_900000002.1`, `type_QC_exception`) and a diversity reference with a quality flag (`GCA_900000005.1`).
- A historical type with no represented accession, and an assignment row (`GCA_900000040.1`) that appears in no other table.
- A CRLF assignments table, which the research reader accepts.

## Contents

- `package/`: 27 files, with `portable.json` as the manifest.
- `portable.json` lists 22 assets. Four records are attested in their own entries rather than in `assets`: the source manifest, the export attestation, the licence manifest and the conversion record.
- Two components, `primary_whole` (5 references) and `genomovar_chromosome` (4 references). Each has a skani index, a sketch database and a marker file under `databases/`, and build and list records under `metadata/`.
- The eleven research tables under `metadata/tables/`.
- `conversion/source-portable.json`: the version 1 manifest, byte for byte.

## Derivation

The fixture was derived in the research project. Each step is recorded below by SHA-256, not by path.

1. `make_synthetic_v1.py` (SHA-256 `70bdd0f3364c5988a01ce391cf8992d609f7041338255a11e568bcf78646bda7`) wrote the synthetic tables, sequences and skani stand-in databases. It then ran the research builder and exporter, pinned to these modules:
   - `evidence.py`: `5b40228ee8f320473e90f61a10e9540724b8bebfd688794235356d2093cf5ee6`
   - `reference_package.py`: `0455b57743f14acc2e472b0546023528f1fdac63967000d9f403b88a8150a34b`
   - `portable_reference.py`: `1ecafd226e4bbd596c22473af2a0b5b51fd0d8b6ee9fa6b00d1c40ecee4745a2`

   The script ran in a private mount namespace with the work directory mounted at `/mnt`. That is why the archived paths in the tables, the skani indexes and the export attestation start with `/mnt/`. They are provenance labels only, and the verifier never opens them.
2. The research loader accepted the version 1 export.
3. `convert_v1_to_v2.py` (SHA-256 `aa2374569bcddd6c7cc50bc1ab305d90f49f01bf1f309d3156fa5393f042cac4`, conversion `terobacter-reference-v1-to-v2/1`) converted it. The disclaimer file had SHA-256 `3e41ffc769ac2a28cd5c00acb00d09e9ad27d2ad9974b5a5ea63da5c25c5872d`. The conversion copied 24 files byte for byte and added the licence manifest, the conversion record and the version 2 manifest.

The real version 2 package came from the same conversion, run on a private copy of the frozen package. The fixture cannot be regenerated from this repository alone, because the builder is a research-project module.

## Not established by this fixture

- skani compatibility. The databases are structurally valid stand-ins: their indexes decode and their offsets cover the sketches. They are not real skani sketches.
- Package and definition dates. The source has none, so the fixture has none.
- Rules for the `partition`, `named_target`, `selection_reason`, `chromosome_original_source` and `chromosome_source_basis` columns. They are carried as written.
- The disclaimer. A real package needs the NCBI wording, and a disclaimer is not approval to distribute.

## Native whole-assembly workflow extension

`tests/assembly.rs` derives a temporary native primary database from this fixture's checked reference labels and metadata. It keeps the committed fixture unchanged. The starting manifest is pinned above as `d594ff403cf479ce677b39ff018cdceb3e5f08582610c6f519e927c16de9f797`.

The first reference uses `common::dna` with seed 42 and length 200,003. Other references mutate this synthetic sequence with xorshift state 927 and an 8% selection rate. The embedded pinned skani seeder and database writer generate actual sketches, markers and index entries. The helper re-lists their byte lengths and hashes, updates their source-manifest records and index entries, and rebinds the export and licence records. The checked package then exercises the public command through native skani to v1 calls. A short plasmid-labelled record is added to the query to verify whole-input scope.

These are synthetic porting fixtures derived from the existing package structure. Full derivation commands and source hashes stay in the research provenance run.

## Native assembly workflow derivatives

`tests/common/assembly.rs` reuses the deterministic native package builder accepted with the whole-assembly v1 implementation, commit `4f5e2de`. Its base sequence is `common::dna(200003, 42)`. Other references carry the same seeded 8% substitutions used by that builder. The v2 witness changes every 32nd base of the base sequence; the foreign-parent veto case changes every 31st base of its competing reference. The unreturned competitor uses the same generator with seed 7654321. The malformed-reference case appends a duplicate identifier after the witness sequence.

These cases rebuild embedded skani sketches, update the synthetic source table, source manifest and export/licence attestations, and create a separately authenticated FASTA component. Every FASTA byte hash and manifest digest is computed from that case; the original committed fixture is preserved. Queries can append a five-base short record to check whole-input coverage. Derivatives exercise integration and failure handling only. They do not estimate biological accuracy. The frozen source fixture is identified by the version 2 manifest digest above; commands, generated hashes and native receipts stay in the research packet.
