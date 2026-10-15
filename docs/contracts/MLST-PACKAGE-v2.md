# Current and custom MLST package contract, version 2

This additive package format uses the existing seven-locus ungapped typing engine.
It makes no new accuracy claim. Historical schema 1 and its frozen cutoff remain
unchanged; the [historical contract](MLST-UNGAPPED-r1.md) still applies to it.

`manifest.json` is authenticated by an independently supplied SHA-256 or a trusted
catalogue's component pin. Version 2 requires these fields:

| Field | Requirement |
|---|---|
| `schema`, `schema_version` | `pterobacter.mlst-scheme/2`, integer `2` |
| `package_id`, `scheme` | Nonempty source-specific identity |
| `package_scope` | `custom` or `current_snapshot` |
| `loci` | Exactly `dnaA`, `fusA`, `gyrB`, `leuS`, `pyrG`, `rplB`, `rpoB`, in that order |
| `retrieved_utc` | Valid UTC `YYYY-MM-DDTHH:MM:SSZ` timestamp |
| `sequence_state`, `currency_statement` | Nonempty description and coverage limitation |
| `source` | Nonempty `name` and nonempty HTTPS `urls` array |
| `rights` | Nonempty `statement`, `access`, HTTPS `terms_url`; `redistribution` is `permitted`, `restricted` or `unknown` |
| `submission_coverage` | `{"kind":"unknown","through":null}` or `{"kind":"through_submission_date","through":"YYYY-MM-DD"}` |
| `data_root`, `profiles_file` | Safe relative directory and a single filename |
| `data_sha256` | Exact mapping of all eight data paths to lowercase SHA-256 |
| `allele_counts`, `profile_count` | Counts matching the verified data |

Each locus file is `data_root/LOCUS.tfa`. The profile table is
`data_root/profiles_file`, with the same columns and allele/ST rules as schema 1.
The package inventory must contain exactly those eight files and `manifest.json`.
Paths use ASCII letters, digits, dots, underscores and hyphens in nonempty segments;
`.` and `..` segments, absolute paths and links are refused.

Version 2 refuses the historical fields `submission_cutoff`,
`complete_through_cutoff_independently_established` and `cutoff_evidence`.
Reports carry the declared coverage separately from retrieval time, plus source,
rights, currency statement and package hashes. A dated coverage declaration is
reported as source-declared and not independently established. Unknown coverage
has a null report cutoff. No metadata field changes the typing or taxonomy rules.

`current_snapshot` describes the source's snapshot claim. It does not prove that
all submissions are present, confer redistribution rights or authorize downloads.
Current PubMLST provisioning is unavailable until source-specific access and
rights are settled. Synthetic tests cover this contract without fetching a new
scheme; users remain responsible for obtaining a permitted, authenticated package.
