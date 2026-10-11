# Evaluation handoff

This page binds the frozen P4a-r3 evaluation design to this release candidate. It maps the design's interface R1–R7 and each `{…}` placeholder in the P4b command plan to a terobacter command, and gives the two hashed tables P4b binds with its argv. It changes nothing in the frozen design, its scorer or its fixtures. P4b binds the source, build and executable digests in its own run before Step 1, as [the P4a-r3 command binding](P4a-r3.md) requires.

| Frozen input | SHA-256 |
|---|---|
| P4a-r3 evaluation design | `4f9357d01fe05d38a6800d615f1cd7a8a13c515c40dcac3cc41998dc54da9247` |
| P4a-r3 reference scorer | `a3bab5d8824cf91d201b640850a7f24d8e2867d694f378524046fef18a4377d3` |
| P4a-r3 scoring cases | `9a93bf22072bcdf5de7664bf1fc7e34311582491b369fcfdcfd57b33427c5720` |
| P4a-r3 GV-4 removal cases | `1434bef5e2f33de2630aa8651193d943d88f3a02749ee7f26b534451cc38206e` |
| P1-r1 acceptance cases | `8b90bdf7c257dd52558a4218e748bd5cf9cb407817997541da4208a1f35ad0bc` |

## Identities

| Item | Value |
|---|---|
| Public commands | `tero` and `terobacter`, the same program under two names |
| Development command | `terobacter-dev`, built with `cargo build --locked --release --features dev --bin terobacter-dev` and never installed by default |
| v1 policy | `conservative-assembly-v1` |
| v2 policy | `conservative-assembly-anim-v2`, contract revision `P1-r1`, `validation_status: not_yet_validated` |
| Genomovar contract | P3a-r1, reported by `p3a-genomovar-contract-r1` |
| skani compatibility profile | `skani-0.3.2-avx2-compatible-dna-v1` |
| ANIm method profile | `reciprocal-mummer4-4.0.1-preserve-n-mum-1-span-v1`, amendment `P1-ANIM-MUMMER4-r1` |
| Reason-vocabulary map | [reason-vocabulary.tsv](reason-vocabulary.tsv), SHA-256 `b988ab4f895741cd57dcaac765707d38fd7187c49f3401deb0b28e66f67431d1` |
| Policy identity | Compiled in. A report's `genomovar.parent.policy_sha256` hashes the exact v1 or v2 policy source, `genomovar.policy_sha256` the P3a reporting contract, and v2 `settings_sha256` the policy mode and thresholds. |
| Scorer field map | [scorer-fields.tsv](scorer-fields.tsv), SHA-256 `2f738d2cba149cf08d1c1c7b16966e73708f2279518185b9ec73f3936d5c611e` |

`tests/p4a_scorer_contract.rs` checks the two map digests above against the files.

## Interface R1–R7

| ID | terobacter command |
|---|---|
| R1 | `tero --reference-package PKG --expected-sha256 PKG_SHA256 [--policy conservative-assembly-anim-v2 --anim-reference-package FASTAS --expected-anim-sha256 FASTAS_SHA256 --anim-output NEW_DIR] [--report-dir NEW_DIR] QUERY.fa`. JSON on standard output is the authoritative report. |
| R2 | `terobacter-dev saved-evidence EVIDENCE --policy conservative-assembly-anim-v2 --reference-package PKG --expected-sha256 PKG_SHA256 [--exclude-references MASK] [--anim-records RECORDS --anim-method-profile PROFILE] [--report-dir NEW_DIR]`. The mask applies to the skani rows, the panel counts and the ANIm competitor set. Records bind by the dry-plan rule only. |
| R3 | `terobacter-dev anim-pair`, as bound in [P4a-r3.md](P4a-r3.md). |
| R4 | `terobacter-dev rescue-plan EVIDENCE --reference-package PKG --expected-sha256 PKG_SHA256 [--exclude-references MASK] [--anim-records RECORDS --anim-method-profile PROFILE]`. It reads evidence and records only and runs no alignment. |
| R5 | `terobacter-dev genomovar-diagnostic --report REPORT --expected-report-sha256 SHA256 --reference-package PKG --expected-sha256 PKG_SHA256 [--mask-member ID]... [--remove-group ID]... [--setting self_alias\|clone_block --exclude-references MASK --exclude-references-sha256 SHA256]`. The deployment `genomovar_id` stays null; `diagnostic_would_assign_genomovar_id` holds the diagnostic candidate. |
| R6 | `terobacter-dev saved-evidence EVIDENCE [--reference-package PKG --expected-sha256 PKG_SHA256]` under the default v1 policy, and the public commands' default `--policy conservative-assembly-v1`. The decision fields are those of the frozen 0.2.0 rule. Saved-evidence output also carries the `versions` block, and v1 its `thresholds`, which the report files render. |
| R7 | Every v2 result carries `trigger`, `call_basis`, `reasons`, `competitor_evaluation`, `legacy_v1_result`, `rescue_status` and `counts` (required, attempted, completed, no-alignment, failed, missing-orientation, not-run). See [v2-decisions.md](../v2-decisions.md#json). |

R2, R4 and a masked v1 run take the primary panel from the authenticated package, so P4b needs no separately built inventory. The package is checked against the whole, unmasked evidence before any mask applies. `--reference-inventory` remains for inputs that carry one. The two cannot be combined.

## P4b command-plan placeholders

| Step | Placeholder | Binding |
|---|---|---|
| 1 | `sha256sum $TOOL {reference package files} {policy files}` | Hash the installed `tero`, `terobacter` and the built `terobacter-dev`, every file under the reference package, and the two maps above. The policies are compiled in: record the source commit, the binary digests and the policy hashes a report carries (see Identities). |
| 1 | `{P3b fixture test command} --p1 … --p3a … --p4a …` | `cargo test --locked --all-features --test v2_p1_acceptance --test genomovar_policy --test genomovar_diagnostics --test p4a_scorer_contract`. `v2_p1_acceptance`, `p4a_scorer_contract` and `genomovar_diagnostics` check their pinned fixtures (the P1-r1 cases, the P4a scoring cases and the GV-4 removal cases) before running. `genomovar_policy` holds the P3a-r1 worker specification's 20 tests and the 1,250 coordinator-review cases, ported case for case. |
| 1 | `$TOOL --version` | Prints `terobacter VERSION`. |
| 1 | RP-3 `fault_injection.py --tool $TOOL` | `tero` with the R1 argv. A failed input reports `execution.status: incomplete` with its stage and exits nonzero: 3 for an unusable assembly, 4 for a package or database failure, 1 for a measurement or ANIm failure. See [assembly.md](../assembly.md). |
| 2 | EN-1 `{TOOL skani-evidence --component primary --query FASTA --output DIR}` | `terobacter-dev skani-search --database PKG/databases/primary_whole --query FASTA`, or the measurement rows of the R1 report for the same query. |
| 2 | EN-2, the 173-member component | `terobacter-dev skani-search --database PKG/databases/genomovar_chromosome --query FASTA --genomovar`. |
| 2 | EN-4 `{TOOL anim-parse --filtered-delta FILE}` | `terobacter-dev anim-parse --filtered-delta FILE --reference-lengths R.json --query-lengths Q.json`, as bound in [P4a-r3.md](P4a-r3.md). |
| 3 | `{TOOL analyze --policy conservative-assembly-v1 --query FASTA --output DIR --format json}` | `tero --reference-package PKG --expected-sha256 PKG_SHA256 --policy conservative-assembly-v1 --report-dir NEW_DIR FASTA`. |
| 3 | `{TOOL analyze --policy conservative-assembly-anim-v2 --anim-plan-only …}` | The same argv with `--policy conservative-assembly-anim-v2` and no `--anim-reference-package`. No ANIm runs. A query whose plan requests no pairs completes with exit 0 and its trigger state: one outside the band, one blocked before ANIm and one with no skani witness. A query whose plan requests pairs exits 1 with `anim_execution.stop_reason: missing_component` and keeps its trigger state and `anim_plan`. |
| 3 | Layer 2 `{TOOL classify-evidence --policy conservative-assembly-v1 --evidence … --exclude-references MASK}` | `terobacter-dev saved-evidence EVIDENCE --reference-package PKG --expected-sha256 PKG_SHA256 --exclude-references MASK`, or `--reference-inventory INVENTORY` in place of the package. The standalone v1 call is computed independently of v2, and under the same mask gives the same status, labels and reasons as v2's `legacy_v1_result`. |
| 4 | `{TOOL rescue-plan --evidence … --exclude-references MASK}` | R4 without `--anim-records`. |
| 6 | `anim_pair_argv` in `inputs/anim-config.json` | R3. The accepted amendment `P1-ANIM-MUMMER4-r1` fixes the profile as `reciprocal-mummer4-4.0.1-preserve-n-mum-1-span-v1`, which R3 records carry. P4b passes that profile to a fresh `plan_anim_waves.py` invocation and records it on Enterobacter#25 before execution. The planner's pair key includes the profile, so the new measurements are never keyed or labelled with the frozen plan's `reciprocal-nucmer3-mum-1-pyani030-span-v1`. The frozen command plan and comparator stay unchanged. This is an execution binding, not a scientific decision. |
| 6d | `{TOOL rescue-plan --evidence … --anim-records … --exclude-references MASK}` | R4 with `--anim-records DIR --anim-method-profile reciprocal-mummer4-4.0.1-preserve-n-mum-1-span-v1`. A directory is read as `.jsonl` files. |
| 7 | `{TOOL classify-evidence --policy conservative-assembly-anim-v2 … --format json,tsv,md}` | R2 with `--report-dir NEW_DIR`, which writes `report.json`, `calls.tsv`, `evidence.tsv` and `report.md`. JSON on standard output matches `report.json`. |
| 7 | EN-5, CO-2 integrated v2 | R1 under v2 with the ANIm component. |
| 7 | CU-2 | R1 or R2 with `--species-ani 96 --subspecies-ani 98`. Custom results report `custom_unvalidated` and never establish a genomovar parent. |
| 8 | `run_genomovar_sets.py` | R5 on the R1 report of each record. |
| 9 | `score.py --records …` | Map each output to the scorer's record with [scorer-fields.tsv](scorer-fields.tsv) and translate reasons with [reason-vocabulary.tsv](reason-vocabulary.tsv). |

## The two maps

`reason-vocabulary.tsv` has one row per reason terobacter can emit, by policy and output field, with the contract reason the scorer reads and the contract that names it. v1 rows map to themselves. v2 `reasons` and `subspecies_reason` rows come from P1-r1. Genomovar rows come from P3a-r1, with three names that differ:

| terobacter reason | P3a-r1 reason |
|---|---|
| `upstream_veto` | `upstream_veto_or_missing_veto_record` |
| `parent_withdrawn_before_publication` | `upstream_veto_or_missing_veto_record` |
| `insufficient_member_coverage` | `insufficient_member_coverage_proposed` |

A parent withdrawn by a late veto maps to the P3a upstream veto, because P3a clears the genomovar whenever the parent is vetoed.

`scorer-fields.tsv` has one row per field the reference scorer reads. It names the terobacter output (`v1-report`, `v2-report`, `parent-report`, `genomovar-section`, `genomovar-diagnostic`) and the JSON path, or says that P4b supplies the field. P4b supplies the manifest state and parent from its R4 `trigger-manifest`, never from the v2 report, and supplies truth, record and dataset IDs and GV-4 bookkeeping. `gv.member_af_pass` is derived from the group rows, and v1 parents report `call_basis` as `skani`. The `genomovar-section` paths are relative: for a deployment record they are read from the report's `genomovar` object, and for an R5 record from the diagnostic output's `diagnostic` object, never from its `baseline` echo.

`tests/p4a_scorer_contract.rs` checks that every reason and field in the 84 scoring cases has a mapping, that every mapped path exists in output from the library's decision, plan and diagnostic functions, and that each terobacter reason maps to exactly one contract reason.

## Known limitations

- The 84 scoring cases and 17 GV-4 cases are checked for mapping and, for GV-4, for diagnostic agreement. The reference scorer is not run in this repository. Its conformance on real terobacter output is evidence for the research run.
- The crate version is set by the release process. A candidate built from a branch reports the version of its source tree, so identify it by commit and binary digest.
- macOS is checked by CI (`just test` and `just install-check` on `macos-latest`). The macOS arm64 platform panel (EN-3) belongs to Enterobacter#30.
- The development commands authenticate the package once at the start. Saved evidence, masks, inventories and ANIm record files are hashed when read and checked again before a result is printed. The public commands recheck the package before publishing, as [assembly.md](../assembly.md) describes.
- Under v1, a reference mask cannot be combined with `--diagnostics`, the bound ANIm method envelope.
- The map tests read output from library functions, not from the command line. The research run replays the P1-r1 cases through `terobacter-dev` and scores them with the reference scorer.
- `rescue-plan` and v2 `saved-evidence` read saved ANIm records and never align. Only R1 under v2 and R3 run MUMmer4.
- Results stay development evidence until the evaluation accepts v2. v2 reports `not_yet_validated` throughout.
