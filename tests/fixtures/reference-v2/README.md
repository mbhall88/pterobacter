# Reference package fixture (`reference-v2/`)

A synthetic version 2 reference package for the authentication tests. It carries no real reference data and is not the frozen package. `tests/common/package.rs` loads it into memory and damages one thing at a time.

## Identity

| Field | Value |
|---|---|
| Package id | `synthetic-enterobacter-reference` |
| Package version | `2026-10-09.test1` |
| SHA-256 of `package/portable.json` | `3c577a111960d9cd756541f1dde165bd2dc8f20048e4ff18cf7e1dcb1a33ea7d` |
| Licence disclaimer | `Synthetic test disclaimer.` |

The digest is pinned as `PINNED_MANIFEST_SHA256` in `tests/common/package.rs`. A test checks the committed manifest against it, so a silent change to the fixture fails the suite.

## Contents

- `package/`: 26 files, with `portable.json` as the manifest.
- `portable.json` lists 22 assets. The source manifest, the export attestation and the licence manifest are attested in their own entries, not in `assets`.
- Two components, `primary_whole` and `genomovar_chromosome`. Each has a skani index, a sketch database and a marker file under `databases/`, and build and list records under `metadata/`.
- Registry, reference and assignment tables under `metadata/tables/`.
- `metadata/source-manifest.json` has a `verified_artifacts` list of 27 entries. The verifier does not use that list as a check. It checks the listed assets against the recipe in the source manifest.

## Derivation

The fixture was generated from synthetic tables by a script that is not part of this repository, so it cannot be regenerated here.

The research project's validator accepts the same files under the version 1 contract. That validator does not read the version 2 schema, so it does not check the version 2 records.

The export attestation's `exporter_source` and `local_validator_source` records give the SHA-256 of two research-project Python modules. Their paths were relabelled to module-relative names, and the fixture does not contain those modules. The digests can be checked only against the research project.

## Not established by this fixture

- Package and definition dates. The source has none, so the fixture has none.
- The `partition`, `qc_pass`, `named_target`, `selection_reason`, `chromosome_original_source` and `chromosome_source_basis` columns, and the evidence `lineage_id`, are not validated against any known rule.
- The disclaimer. A real package needs the NCBI submitter-rights wording.
- Equivalence with the frozen 0.2.0 package. This is a stand-in. The frozen package still needs a versioned version 2 conversion before it can be authenticated.
