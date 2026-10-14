# Terobacter report

This summary is derived from the JSON report, which is authoritative. Assemblies: 3.

## Assembly 1: isolate.fa

### Final call

**Call status:** assigned

- **Species:** assigned — Enterobacter syntheticus (SYN-SP01)
- **Subspecies:** assigned — Enterobacter syntheticus subsp. syntheticus (SYN-SS01)
- **Genomovar:** none reported
- **Reasons:** —
- **Subspecies reason:** diagnostic_candidate_only
- **Nearest parent (not an assignment):** species SYN-SP01, skani ANI 100.0 %

### Analysis settings and execution

- ANI thresholds: standard
- Validation status: —
- Minimum species ANI: 96
- Minimum subspecies ANI: 98
- Minimum aligned fraction (both directions): 80
- Subspecies margin (percentage points): 1
- Whole-assembly ANI method: skani
- Execution: complete at stage genomovar

### Initial skani result and ANIm

- Initial skani result: this is the final result; ANIm is disabled.
- ANIm: disabled
- skani panel: 5 references, 5 returned, 0 not returned

### Strongest matches

The five highest skani references are shown, plus every exact tie at fifth place. Rows in one tie group are ordered by reference ID for display only; the order does not choose a winner. Each reference is evidence, not a vote. ANI methods are not mixed in one ranking: skani orders this table and any ANIm values are shown beside it.

| Tie group | Reference | Parent | skani ANI % | AF query % | AF reference % | ANIm identity QR/RQ % |
|---|---|---|---|---|---|---|
| 0 | primary_whole:A | species SYN-SP01 | 100.0 | 99.0 | 99.0 | — |
| 1 | primary_whole:B | species SYN-SP02 | 91.38 | 99.0 | 99.0 | — |

### Whole-panel species and lineage summary

| Parent | Panel references | Returned | Best skani ANI % | Among strongest matches | Challenging references |
|---|---|---|---|---|---|
| species SYN-SP01 | 3 | 3 | 100.0 | yes | — |
| species SYN-SP02 | 2 | 2 | 91.38 | yes | — |

### Challenges

With ANIm disabled, there is no alignment-based foreign-parent challenge test, so none is listed. Every other parent's best skani ANI is in the whole-panel summary above; compare it with the call.

### Supplementary results

- Historical MLST: not_requested
- Genomovar search: withheld

### Provenance

- Query SHA-256: `3fa07c219f750c844d195aaf6f5802748b9025ef5c9d0af7f7a0367d326e52ff`
- Bases: 200008; records: 2
- Evidence SHA-256: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
- Reference package 2026-10-09.test2, manifest SHA-256 84f87508cf27391c97ed753af11855955bda371b561c38e4aeb22c2c5bd0b459
- Software version: 0.1.0. Exact schema, method and analysis identities are retained in report.json.

### What to do next

The assignment is conditional on the approved rules and the supplied sequence. It does not establish that the isolate is pure.

## Assembly 2: bad name .fa

### Final call

**Call status:** not_evaluated

- **Species:** not evaluated
- **Subspecies:** not evaluated
- **Genomovar:** none reported
- **Reasons:** —

### Analysis settings and execution

- ANI thresholds: standard
- Validation status: —
- Minimum species ANI: —
- Minimum subspecies ANI: —
- Minimum aligned fraction (both directions): —
- Subspecies margin (percentage points): —
- Whole-assembly ANI method: skani
- Execution: incomplete at stage input
- Execution error: input must be assembly FASTA, not FASTQ

### Initial skani result and ANIm

- Initial skani result: this is the final result; ANIm is disabled.
- ANIm: disabled
- skani panel: — references, — returned, — not returned

### Strongest matches

The five highest skani references are shown, plus every exact tie at fifth place. Rows in one tie group are ordered by reference ID for display only; the order does not choose a winner. Each reference is evidence, not a vote. ANI methods are not mixed in one ranking: skani orders this table and any ANIm values are shown beside it.

No skani reference was returned.

### Whole-panel species and lineage summary

No panel summary was produced for this report.

### Challenges

With ANIm disabled, there is no alignment-based foreign-parent challenge test, so none is listed. Every other parent's best skani ANI is in the whole-panel summary above; compare it with the call.

### Supplementary results

- Historical MLST: not_requested
- Genomovar search: —

### Provenance

- Query SHA-256: ``
- Bases: —; records: —
- Evidence SHA-256: —
- Reference package —, manifest SHA-256 —
- Software version: 0.1.0. Exact schema, method and analysis identities are retained in report.json.

### What to do next

The analysis did not complete, so no biological conclusion can be drawn from this report. Fix the execution error above and run again; retained evidence is in the JSON report.

## Assembly 3: borderline.fa

### Final call

**Call status:** assigned

- **Species:** assigned — Enterobacter syntheticus (SYN-SP01)
- **Subspecies:** unresolved (no assignment)
- **Genomovar:** none reported
- **Reasons:** —
- **Subspecies reason:** ANIm_species_rescue_does_not_establish_subspecies
- **Call basis:** ANIm_assisted
- **Nearest parent (not an assignment):** Enterobacter syntheticus (species SYN-SP01), skani ANI 95.29 %

### Analysis settings and execution

- ANI thresholds: standard
- Validation status: not_yet_validated
- Minimum species ANI: 96
- Minimum subspecies ANI: 98
- Minimum aligned fraction (both directions): 80
- Subspecies margin (percentage points): 1
- Conflict ANI: 96
- Trigger lower ANI: 94.5
- Whole-assembly ANI method: skani
- Boundary ANI method: ANIm using MUMmer4
- Execution: complete at stage genomovar

### Initial skani result and ANIm

- Initial skani result: unresolved (species —; reasons nearest_below_species_ANI)
- Trigger: true (triggered_with_witness_candidate)
- Rescue state: rescued
- Competitor panel: complete
- Challenge check: completed
- ANIm pairs: 5 required, 5 attempted, 5 completed, 1 no alignment, 0 failed, 0 not run
- skani panel: 5 references, 4 returned, 1 not returned

### Strongest matches

The five highest skani references are shown, plus every exact tie at fifth place. Rows in one tie group are ordered by reference ID for display only; the order does not choose a winner. Each reference is evidence, not a vote. ANI methods are not mixed in one ranking: skani orders this table and any ANIm values are shown beside it.

| Tie group | Reference | Parent | skani ANI % | AF query % | AF reference % | ANIm identity QR/RQ % |
|---|---|---|---|---|---|---|
| 0 | primary_whole:A | species SYN-SP01 | 95.29 | 99.0 | 99.0 | — |

### Whole-panel species and lineage summary

| Parent | Panel references | Returned | Best skani ANI % | Among strongest matches | Challenging references |
|---|---|---|---|---|---|
| species SYN-SP01 | 3 | 3 | 95.29 | yes | — |

### Challenges

No reference challenged the call. The competitor panel was fully measured.

### Supplementary results

- Historical MLST: not_requested
- Genomovar search: withheld

### Provenance

- Query SHA-256: `3fa07c219f750c844d195aaf6f5802748b9025ef5c9d0af7f7a0367d326e52ff`
- Bases: 200008; records: 2
- Evidence SHA-256: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
- Reference package 2026-10-09.test2, manifest SHA-256 84f87508cf27391c97ed753af11855955bda371b561c38e4aeb22c2c5bd0b459
- Software version: 0.1.0. Exact schema, method and analysis identities are retained in report.json.

### What to do next

The assignment is conditional on the approved rules and the supplied sequence. It does not establish that the isolate is pure.
