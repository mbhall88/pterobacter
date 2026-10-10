# Genomovar diagnostic fixtures

`cases.json` contains 17 synthetic saved-evidence perturbation cases from the accepted P4a-r3 design. The cases use metric rows and short member IDs, not genomes or isolate accessions. They were built through the accepted P3a model.

| Source | SHA-256 |
|---|---|
| GV-4 perturbation cases | `1434bef5e2f33de2630aa8651193d943d88f3a02749ee7f26b534451cc38206e` |

Tests check this pin before applying the diagnostic assessment. The 12 applicable cases are compared with their expected candidate and incomplete groups. Evidence-row exclusions are passed as declared operations to the assessment; required membership is retained. The three refused controls and two not-applicable controls are not sent through the diagnostic: tests check their recorded baseline summaries and verify the stated refusal or applicability preconditions from the supplied group membership and metric rows. They do not manufacture diagnostic results for controls that lack the accepted prerequisites.

`tests/genomovar_diagnostic_cli.rs` exercises the executable with synthetic metrics and an authenticated frozen package supplied through `TEROBACTER_FROZEN_PACKAGE` and `TEROBACTER_FROZEN_PACKAGE_SHA256`. Run it with `--all-features -- --ignored`. It checks evidence-only removals, matching primary masks, retained and lost parent eligibility, supplied warnings, and binding failures under v1 and v2. The package is used for metadata and verification; no database search runs. The test is ignored in ordinary CI because the frozen package is not distributed with this repository. Its commands and results belong in the private provenance run.
