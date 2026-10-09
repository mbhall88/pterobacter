# Saved-evidence v1 decisions

The policy `conservative-assembly-v1` implements the frozen 0.2.0 species and subspecies rules over saved skani 0.3.2 evidence. The decision core is pure: `SavedEvidence::from_json`, optional `with_annotations`, then `decide`. The development binary owns files and verifies their hashes. Whole-assembly execution is a separate implementation ticket.

## Calling rules

A species requires a unique raw-nearest parent, a named species, a measured alternative parent, and a reference in that species with ANI ≥96% and both AFs ≥80%. The witness need not be the raw-nearest reference. An exact cross-parent ANI tie withholds. Parent identity is the species ID, else the lineage ID, else the individual reference ID.

A subspecies requires a species rank candidate, a unique raw-nearest group within that parent, a named subspecies, ANI ≥98%, both AFs ≥80%, a measured within-parent alternative, and a margin ≥1 percentage point. One reference must satisfy ANI, AF and the margin together. All raw other groups count as competitors, including species-only groups, low-AF rows and foreign parents. Missing comparisons do not become zero ANI or infinite margins.

Both ranks use skani's emitted two-decimal values, represented as integer hundredths. Extra decimals are rejected rather than rounded. A declared AF of `0.8` means **0.8%**.

The two-parent alert withholds when references from two named species each have ANI ≥96% and reference AF ≥80%, even if query AF is low. An unexplained scope assessment, a supplied method-gate disagreement, or a supplied external problem warning also withholds. All simultaneous reasons remain in `blocking_reasons`. The frozen scope invariant keeps `assignment.reason` equal to `unexplained_residual_sequence` when that veto applies.

## JSON

The schema is `terobacter.assembly-call/1`. `assignment` contains the final frozen v1 fields; every taxon field is null when the species is unresolved. `species` and `subspecies` provide rank summaries. `rank_evidence` keeps raw nearest hits, qualifying witnesses, all competitor groups and subspecies gaps, separately from accepted labels.

`scope_assessment`, `two_parent_alert`, `method_diagnostics` and `external_problem_warnings` retain supplied observations and concern text. A missing optional assessment is explicit; a match does not establish purity or explain residual sequence. The 95–96% near-boundary note is report-only and uses a raw-nearest reference with both AFs ≥80%.

All reason codes map directly to the frozen vocabulary in [v1-reasons.tsv](v1-reasons.tsv). Semicolon-separated assignment reasons are compositions of those codes. Scope explanations and supplied concern text remain free text.

## Optional annotations

```sh
terobacter-dev saved-evidence evidence.json --evidence-sha256 HASH \
  --diagnostics diagnostics.json --diagnostics-sha256 HASH \
  --reference-inventory inventory.json --reference-inventory-sha256 HASH \
  --problem-warnings warnings.json --problem-warnings-sha256 HASH
```

Each envelope binds the exact evidence bytes, query digest and reference-package digest. Diagnostics use `1.0-shadow-diagnostics` and require a `1.0-shadow-primary-inventory` from a checked package. Problem warnings use `1.0-shadow-problem-warnings`. Paths and SHA-256 digests identify the supporting artifacts. The binary checks the original input and artifact hashes again before printing a decision; failed checks produce no decision.

Under v1, each reciprocal ANIm direction is compared with that reference's saved skani triple at ANI ≥96% and both AFs ≥80%. Any gate disagreement withholds, including support for another parent. Method diagnostics retain their decimal precision; they are not skani measurements. Positive rescue belongs to v2.

The pure interface validates bindings and metadata. Its output explicitly says that external artifact bytes and package authenticity require a file wrapper and a checked package. The development command does not authenticate an inventory merely because its SHA-256 matches a caller-supplied value.

## Supported and historical interfaces

The supported input is an unassigned `2.0.0-dev-evidence` assembly record with null taxon fields, complete ranked evidence and declared percent units. Only whole-input skani 0.3.2 is accepted for this policy. The two recorded query-digest layouts are supported; conflicting digests are rejected. References without a species or lineage must use the `context_only` role.

The historical Python `legacy_cli` is outside the supported release API. Its inference that AF `0.8` means 80% and its retention of taxon labels in unresolved output are not reproduced. The frozen 0.2.0 wheel and its evidence-only route remain external comparators, unchanged by this port.

Parity on already exposed native and replay records is a software compatibility check. It does not establish independent biological accuracy or satisfy the full RG-1 evaluation gate.
