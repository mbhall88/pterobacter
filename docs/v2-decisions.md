# Saved-evidence v2 decisions and the dry rescue plan

The policy `conservative-assembly-anim-v2` implements the P1-r1 calling contract: a species-only ANIm rescue for queries just below the v1 species threshold. It is decided from saved skani 0.3.2 evidence plus saved reciprocal ANIm pair records. No alignment runs here. v2 reports `validation_status: not_yet_validated` until the evaluation accepts it.

The decision core is pure, like v1. A front end supplies text:

1. `SavedEvidence::from_json`, then `with_annotations` for bound problem warnings.
2. `Panel::from_json`: the primary inventory, checked against the evidence. It lists every primary reference, including those skani did not return.
3. `RescueInput::new(evidence, panel, mask, thresholds)`. The mask removes references from the skani rows, the panel counts and the ANIm competitor set. The legacy v1 result is computed on the masked evidence too.
4. `AnimRecords::from_jsonl` (or `from_json` for one record), then `bind_to_plan` (or `bind_supplied`).
5. `decide` for the v2 result, or `plan` for the dry rescue plan.

## Calling rules

Outside the trigger band the v1 skani rules apply unchanged, including the subspecies rule. Any supplied ANIm for a returned reference is checked against the old method gate (ANI ≥ T, both AFs ≥ 80), and a disagreement withholds the call.

The trigger is 94.5 ≤ maximum returned skani ANI < 96.0. Inside it:

- A hard veto (problem warning, unexplained scope, two-parent alert) or a structural reason (cross-parent nearest tie, unnamed nearest, no measured contrast) withholds before any ANIm is needed: `competitor_evaluation: not_run_already_blocked`.
- Witness candidates are references in the raw-nearest named species with skani ANI in the band and both skani AFs ≥ 80, ordered by decreasing skani ANI, then reference ID. With none, the call is withheld with `skani_coverage_not_rescuable`.
- A witness needs both ANIm identities strictly above T and all four directional AFs ≥ 80 on the same reference. A missing or failed witness measurement gives `ANIm_incomplete`, never `ANIm_no_strict_witness`.
- With a witness, every reference in the masked panel needs a finished reciprocal pair. Missing or failed pairs give `ANIm_competitor_incomplete`.
- Another parent with skani or either ANIm orientation at ≥ min(96, T) and reference AF ≥ 80 is a foreign-parent challenge. Query AF has no floor here. Challenges are reported even when another veto settled the result.
- The old method-gate disagreement still vetoes, except for one case: a witness candidate that fails skani only on ANI and passes the strict ANIm gate. That case is marked `expected_ANI_method_crossing`.
- A rescue sets the species only. The subspecies is null with `ANIm_species_rescue_does_not_establish_subspecies`.

ANIm gates use exact integer cross-multiplication on D, E, covered bases and lengths. A successful orientation with no alignment is a measured, non-supporting result. Any other orientation status is a failure.

skani comparisons use the emitted two-decimal values. A threshold T compares to a skani value through its two-decimal ceiling, which gives the same answer for every value skani can emit.

## Binding saved ANIm

A record is bound when its query SHA-256 matches the evidence and its reference ID and SHA-256 match the panel. Its method profile must match the expected profile. Every record for one query must agree on the query length. Records for other queries are skipped and counted, and masked references are excluded. A record with no query or reference binding is counted as unattributed and never bound, so its pair stays `not_run`. A stale hash, wrong profile, conflicting duplicate or malformed metrics is an error.

`bind_to_plan` applies the evaluation's binding rule on top. It keeps only the pairs the dry plan requests under the same evidence, mask and thresholds. Witness records decide whether the competitor panel is requested, and every other record is excluded with `not_requested_by_dry_plan`. An out-of-band or already-blocked record therefore binds no ANIm. `bind_supplied` keeps every provenance-bound record, which is how P1 treats supplied evidence; the P1 acceptance cases run through it.

Records use the `terobacter-anim-pair-v1` format from the per-pair ANIm operation (issue #8). Both bindings must cover the whole assembly, and the reference must come from the `primary_whole` component. Each orientation must name the native reference and query its direction implies: the reference is the native reference in `QR`, and the query in `RQ`. Orientation status `aligned` or `no_alignment` is a measurement, `error` or `not_run` a failure, and any other status an error. Receipts and native artifacts are not read.

## Dry rescue plan

`plan` reports the trigger state and `max_skani_ani_percent`, then the nearest parent, the witness candidates and the requested pairs with their status: `outstanding`, `measured`, `failed` or `missing_orientation`. Requested pairs come in witness order, then the rest of the panel by reference ID. It also lists the outstanding witnesses or competitors, known challenges, decisive vetoes and counts. `stage` is `not_triggered`, `blocked_before_anim`, `no_skani_witness`, `witness` or `competitor_panel`. Trigger states use the evaluation's names: `no_estimate`, `at_or_above_96_v1_route`, `below_band_v1_route`, `triggered_cross_parent_tie`, `triggered_unnamed_nearest`, `triggered_no_measured_contrast`, `triggered_no_skani_witness` and `triggered_with_witness_candidate`. Under custom thresholds, the 96 in a state name stands for T.

## JSON

The schema is `3.0.0-conditional-assembly-report`. Every v2 result carries the following fields:

- `policy_id`, `contract_revision`, `policy_mode`, `validation_status` and `thresholds`.
- `status`, `species_id`, `subspecies_id`, `subspecies_reason`, `call_basis` (`skani`, `ANIm_assisted` or null) and `reasons`.
- `trigger`, `trigger_state`, `nearest_parent`, `rescue_status` (`not_triggered`, `rescued`, `incomplete` or `withheld`), `competitor_evaluation` and `evaluation_status`.
- `counts`: required, attempted, completed, no-alignment, failed (an orientation failed), missing-orientation (a direction has no record) and not-run reciprocal pairs, plus pairs supplied outside the plan. Attempted pairs are completed, failed or missing an orientation.
- `witness`, `witness_assessment`, `challenges`, `challenge_evidence` and `method_diagnostics`. Each reported ANIm pair gives both identities, all four AFs and the conservative minima (`minimum_identity_percent`, `minimum_af_percent`), which stay null unless both orientations measured them.
- `top_references`: the top five skani references and every tie at fifth place, with display-only tie groups.
- `panel_summary`: one row per parent across the whole masked panel.
- `legacy_v1_result`: the unmodified v1 outcome on the same masked evidence.

`input` records the evidence, inventory and query hashes, the mask and how each ANIm record was bound.

Reason codes map to the contract vocabulary in [v2-reasons.tsv](v2-reasons.tsv).

## Development binary

```sh
terobacter-dev rescue-plan evidence.json --reference-inventory inventory.json \
  [--anim-records records.jsonl|records/ --anim-method-profile PROFILE] \
  [--exclude-references mask.txt] [--problem-warnings warnings.json --problem-warnings-sha256 HASH]

terobacter-dev saved-evidence evidence.json --policy conservative-assembly-anim-v2 \
  --reference-inventory inventory.json [--reference-inventory-sha256 HASH] \
  [--anim-records records.jsonl|records/ --anim-method-profile PROFILE] \
  [--exclude-references mask.txt]
```

`--anim-records` takes JSON Lines files, single-record `.json` files or directories of `.jsonl` files, and both commands bind records by the dry-plan rule. A mask file lists one reference ID per line. `rescue-plan` takes no sequence input and runs no alignment. A valid decision exits 0 even when the rescue is incomplete; `rescue_status` and `evaluation_status` say so. v1 refuses `--anim-records` and `--exclude-references`.

## Not covered here

- Command-line threshold overrides, with their messages and provenance digest, belong to issue #10. The core already accepts custom thresholds, because six P1 cases use them.
- Running skani or ANIm for a whole assembly, and nonzero exits for incomplete required work, belong to issue #11.
