# Genomovar reporting

A genomovar is an informal chromosome ANI group within *Enterobacter xiangfangensis* subsp. *hoffmannii* (`ECTX-SS007`, Group A), the project's proposed combination. The definition is frozen as `2026-10-02-v1`: 25 groups (GV01 to GV25) of 173 historical chromosome members. Each group was built by complete linkage at chromosome ANI ≥99.50%. A genomovar call matches a query to one frozen group under the accepted P3a contract, `p3a-genomovar-contract-r1`. It is an operational match, not a population boundary, a purity estimate or a new taxon.

Every whole-assembly report has a `genomovar` section. `assignment.genomovar_id` is set only when that section's status is `assigned`, and only after the final recheck described below.

## When a genomovar is assigned

All of these must hold:

1. **Parent:** The same query is actually assigned species `ECTX-SP025` and subspecies `ECTX-SS007` by an accepted parent call, with no blocking reason and a scope outcome of `not_assessed` or `explained_after_checks`. A v1 call counts when it was decided against the authenticated package. A v2 call counts only when it is the unchanged v1 skani call: default thresholds, a complete evaluation, `call_basis: skani`, and the same species and subspecies as its `legacy_v1_result`. An ANIm species rescue sets subspecies to null and never supports a genomovar. Custom thresholds do not either, even at the default values.
2. **Definition:** The package carries exactly the frozen definition: the 25 groups and 173 members under `ECTX-SS007`, members equal to the `genomovar_chromosome` component, and the pinned membership SHA-256 `1a2202dde4e0afcf78a89c1e6c084563169bd5cdea0f102da8113b1a87ebf0ab`. That digest covers `group<TAB>member<LF>` lines sorted by group, then member.
3. **All members:** The reported skani ANI is ≥99.50% to every member of exactly one group. The threshold is inclusive. Values are compared exactly as the two-decimal estimates skani emits, with no rounding or epsilon.
4. **Rival exclusion:** Every other group has at least one measured member below 99.50%. A member that skani did not return is not a negative measurement, so a group with a missing member and no measured failure stays a possible rival and blocks the call. Two or more compatible groups overlap; nothing breaks the tie, not the best ANI, a medoid, reference counts or ST.
5. **Member coverage:** Both the query and the reference aligned fraction are ≥80% to every member of the compatible group. 80.00% passes and 79.99% does not. A low-AF compatible rival still counts as a rival.

If a gate fails, the genomovar is withheld and the parent call stays as it is. The section keeps the compatible and incomplete groups, each group's state, missing and failing members, and the ANI and AF minima.

## The genomovar search

The search compares the whole assembly as supplied (`query_scope: whole_assembly_as_supplied`) with the 173 historical chromosome members (`reference_scope: historical_chromosome_record_set`). It uses the embedded skani 0.3.2 with the genomovar settings `c125_m1000_learnedANI_s80_minAF0_uncapped`: compression 125, marker compression 1000, learned ANI, screen 80%, no AF reporting floor and no result cap. These are not the primary search's settings. Primary-search thresholds and custom v2 thresholds do not change the genomovar definition or its rule.

The search runs only for an eligible parent on a package with the frozen definition. Otherwise `search.status` is `not_run`. The genomovar database is opened on first use, so a batch with no Group A parent never loads it. `search.member_evidence` lists every returned member comparison, and `search.not_returned_reference_ids` lists the members skani did not return.

## Report fields

| Field | Meaning |
|---|---|
| `status` | `assigned`, `withheld`, or `failed` when the search could not complete. |
| `genomovar_id` | The assigned group's concept ID, for example `ECTX-GV-2026-10-02-v1-GV03`, otherwise null. |
| `reasons` | Every gate that failed. One reason never hides another. |
| `compatible_groups`, `incomplete_groups` | Groups with every member ≥99.50%, and groups with no measured failure but a missing member. |
| `groups` | Per group: `state` (`compatible`, `incompatible`, `incomplete` or `unassessable`), required and observed member counts, `missing`, `below_threshold`, `below_member_af` and the minima. |
| `warnings` | Always the operational-match warning. A singleton group adds a sparse-reference warning. A minimum ANI of exactly 99.50% adds a cutoff warning. |
| `parent` | The parent call the section was assessed against. |
| `search` | Engine, settings, scopes, query and package hashes, counts and member evidence. |
| `definition`, `rule` | The definition identity and the gate values. |

| Reason | Meaning |
|---|---|
| `absent_eligible_parent` | No actually assigned, verified Group A parent from an accepted policy. |
| `unresolved_scope` | The parent's scope outcome is `unexplained_after_checks`. |
| `upstream_veto` | The parent call carries a blocking reason. |
| `unsupported_scope` | Not a whole assembly, or not the historical chromosome references. |
| `unsupported_method_or_definition` | Not the pinned engine, settings or frozen definition. `detail` says which. |
| `incomplete_or_unbound_execution` | The search did not complete, or its query or package differs from the parent's. |
| `diagnostic_only_no_deployment_call` | The contract's leave-one-out diagnostic mode. The command never runs it. |
| `overlapping_definitions` | Two or more groups are compatible. |
| `incomplete_alternative_or_member_evidence` | A group has a missing member and no measured failure. |
| `no_compatible_definition` | No group is compatible and none is incomplete. This is not a new genomovar. |
| `insufficient_member_coverage` | The one compatible group has a member with an AF below 80%. |
| `parent_withdrawn_before_publication` | The parent changed or was vetoed after assessment. |

## Recheck before publication

Immediately before writing reports, the command rechecks each assigned genomovar against its report's final parent. The report must still be complete, and its parent call must be unchanged, eligible and bound to the same query and package. A later failure, such as the final package check, clears the genomovar with `parent_withdrawn_before_publication` and keeps the evidence.

A genomovar search failure is an execution failure, not a negative result. As with any other failed stage, the report sets `execution.status: incomplete` at stage `genomovar` with null assignment fields. It keeps the measurement, the evidence and a `failed` genomovar section that records the parent call, and the command exits 1.

## Limits

The 15 singleton groups have no independently demonstrated population boundary. Identical sequence is not evidence of isolate identity. Nothing reclusters the definition, changes a membership, updates the package from a query or uses ST as a tiebreaker. Queries matching the same group are not necessarily ≥99.5% to each other. Skani aligned fraction does not certify exact whole-input base coverage, and records shorter than 500 bases are not sketched. These are implementation checks against the accepted contract; the evaluation of genomovar calls on new genomes is separate.
