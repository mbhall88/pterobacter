# Genomovar reporting

Look for a genomovar only after an eligible species and subspecies call. In this reference package the definitions cover the proposed *Enterobacter xiangfangensis* subsp. *hoffmannii*: 25 informal chromosome ANI groups built from 173 historical chromosome members. They are not formal taxonomic names.

A match requires sufficient ANI and coverage to every member of exactly one definition, with measured evidence excluding the alternatives. The query remains the whole assembly as supplied; the reference definition uses historical chromosome record sets. A missing rival comparison can block the match.

A unique result means compatibility with the supplied definition. An unrepresented group can match a neighbouring group, so the result does not prove that the query's source group is present in the database. Two compatible groups are withheld, rather than choosing the nearest one. Fifteen of the definitions have only one member; matching one carries a sparse-reference warning and does not establish its population boundary.

Read the `genomovar` section for the result, reasons, warnings and member comparisons. A withheld genomovar does not by itself erase a supported species or subspecies call. A candidate ANIm species rescue supports neither subspecies nor genomovar; custom thresholds do not support a genomovar either.

Two queries with the same result are not necessarily at least 99.5% ANI to each other. ST does not break a genomovar tie. Exact gates and reason codes follow below. For deeper investigation, use [boundary review](boundary-review.md).

## What is a genomovar?

Rosselló and colleagues introduced the term for genomic groups within a named species in their 1991 study, [Genotypic and phenotypic diversity of *Pseudomonas stutzeri*](https://doi.org/10.1016/S0723-2020(11)80294-8). For the more recent use of 99.5% ANI to describe within-species units, see [Rodriguez-R et al. (2024)](https://doi.org/10.1128/mbio.02696-23). The definition below is specific to this reference collection.

## Definition and reporting contract

A genomovar is an informal chromosome ANI group within *Enterobacter xiangfangensis* subsp. *hoffmannii* (`ECTX-SS007`, Group A), the project's proposed combination. The definition is frozen as `2026-10-02-v1`: 25 groups (GV01 to GV25) of 173 historical chromosome members. Each group was built by complete linkage at chromosome ANI ≥99.50%. A genomovar call matches a query to one frozen group under the accepted P3a contract, `p3a-genomovar-contract-r1`. It is an operational match, not a population boundary, a purity estimate or a new taxon.

Every whole-assembly report has a `genomovar` section. `assignment.genomovar_id` with `--no-anim`, or the top-level `genomovar_id` with ANIm enabled, is set only when that section's status is `assigned`, and only after the final recheck described below.

## When a genomovar is assigned

All of these must hold:

1. **Parent:** The same query is actually assigned species `ECTX-SP025` and subspecies `ECTX-SS007` by an accepted parent call, with no blocking reason and a scope outcome of `not_assessed` or `explained_after_checks`. Either call counts only when it was decided against the authenticated package (`reference_package.status: verified`). With ANIm enabled, the call must still match the initial skani call: default thresholds, a complete evaluation, `call_basis: skani`, and the same species and subspecies as its `legacy_v1_result`. An ANIm species rescue sets subspecies to null and never supports a genomovar. Custom thresholds do not either, even at the default values.
2. **Definition:** The package carries exactly the frozen definition: the 25 groups and 173 members under `ECTX-SS007`, members equal to the `genomovar_chromosome` component, and the pinned membership SHA-256 `1a2202dde4e0afcf78a89c1e6c084563169bd5cdea0f102da8113b1a87ebf0ab`. That digest covers `group<TAB>member<LF>` lines sorted by group, then member.
3. **All members:** The reported skani ANI is ≥99.50% to every member of exactly one group. The threshold is inclusive. Values are compared exactly as the two-decimal estimates skani emits, with no rounding or epsilon.
4. **Rival exclusion:** Every other group has at least one measured member below 99.50%. A member that skani did not return is not a negative measurement, so a group with a missing member and no measured failure stays a possible rival and blocks the call. Two or more compatible groups overlap; nothing breaks the tie, not the best ANI, a medoid, reference counts or ST.
5. **Member coverage:** Both the query and the reference aligned fraction are ≥80% to every member of the compatible group. 80.00% passes and 79.99% does not. A low-AF compatible rival still counts as a rival.

If a gate fails, the genomovar is withheld and the parent call stays as it is. The section keeps the compatible and incomplete groups, each group's state, missing and failing members, and the ANI and AF minima.

## The genomovar search

The search compares the whole assembly as supplied (`query_scope: whole_assembly_as_supplied`) with the 173 historical chromosome members (`reference_scope: historical_chromosome_record_set`). It uses the embedded skani 0.3.2 with the genomovar settings `c125_m1000_learnedANI_s80_minAF0_uncapped`: compression 125, marker compression 1000, learned ANI, screen 80%, no AF reporting floor and no result cap. These are not the primary search's settings. Primary-search thresholds and custom ANI thresholds do not change the genomovar definition or its rule.

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
| `parent` | The parent call, calling-rule source digest and parent decision digest before attachment. |
| `policy_sha256`, `approval` | Digest of the base and accepted addendum identities, with approval source and contract hashes. |
| `search` | Engine, settings, scopes, query and package hashes, counts and member evidence. |
| `definition`, `rule` | The definition identity and the gate values. |

| Reason | Meaning |
|---|---|
| `absent_eligible_parent` | No actually assigned, verified Group A parent under the standard calling rules. |
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

The reporting-rule digest hashes the two contract SHA-256 strings in order, each followed by a newline. `approval` records those identities and Michael's dated decision source. The parent-rule digest hashes the exact skani decision source; with ANIm enabled it also includes alignment decision, threshold and pair-binding sources, separated by newlines. The parent result digest hashes compact JSON with sorted keys after removing `genomovar`, `mlst`, `genomovar_id`, `execution`, `measurement`, `evidence`, `anim_execution`, `anim_plan`, `implementation` and nested `assignment.genomovar_id`. MLST is an independent supplementary receipt: attaching a not-requested, successful or failed MLST result does not change the parent taxonomic decision or its digest. The retained decision contains the query/package/evidence identities, parent assignment and vetoes. Paths and timing are not part of either rule digest.

## Recheck before publication

Immediately before writing reports, the command rechecks each assigned genomovar against its report's final parent. The report must still be complete, and its parent call must be unchanged, eligible and bound to the same query and package. A later failure, such as the final package check, clears the genomovar with `parent_withdrawn_before_publication` and keeps the evidence.

A genomovar search failure is an execution failure, not a negative result. As with any other failed stage, the report sets `execution.status: incomplete` at stage `genomovar` while retaining independently supported species and subspecies fields. Final package checks still apply to those retained calls. It keeps the measurement, the evidence and a `failed` genomovar section that records the parent call, and the command exits 1.

## Limits

An unrepresented group can match a neighbouring reference group. A unique match means compatibility with the supplied definition; it does not prove that the query's source group is represented or that all unsampled groups have been excluded. The frozen omission diagnostics observed this limitation between GV01/GV24 and GV04/GV05. With the full reference set, overlapping compatible groups remain withheld. Singleton matches retain the sparse-reference warning. These observations do not justify changing the frozen threshold after seeing the results.

The 15 singleton groups have no independently demonstrated population boundary. Identical sequence is not evidence of isolate identity. Nothing reclusters the definition, changes a membership, updates the package from a query or uses ST as a tiebreaker. Queries matching the same group are not necessarily ≥99.5% to each other. Skani aligned fraction does not certify exact whole-input base coverage, and records shorter than 500 bases are not sketched. These are implementation checks against the accepted contract; the evaluation of genomovar calls on new genomes is separate.
