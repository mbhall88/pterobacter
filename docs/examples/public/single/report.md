# Pterobacter report

This summary is derived from the JSON report, which is authoritative. Assemblies: 1.

## Assembly 1: RHBSTW\-00333

**Input file:** `GCA_013738375.1.fna`

### Final call

**Call status:** assigned

- **Species:** assigned — Enterobacter xiangfangensis (ECTX-SP025)
- **Subspecies:** assigned — Enterobacter xiangfangensis subsp. hoffmannii (ECTX-SS007)
- **Genomovar:** ECTX-GV-2026-10-02-v1-GV10
- **Reasons:** —
- **Subspecies reason:** diagnostic_candidate_only
- **Call basis:** skani
- **Nearest parent (not an assignment):** species ECTX-SP025, skani ANI 100.0 %

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

- Initial skani result: assigned (species ECTX-SP025; reasons —)
- Trigger: false (at_or_above_96_v1_route)
- Rescue state: not_triggered
- Competitor panel: not_required
- Challenge check: not_applicable
- ANIm pairs: 0 required, 0 attempted, 0 completed, 0 no alignment, 0 failed, 0 not run
- skani panel: 140 references, 140 returned, 0 not returned

### Strongest matches

The five highest skani references are shown, plus every exact tie at fifth place. Rows in one tie group are ordered by reference ID for display only; the order does not choose a winner. Each reference is evidence, not a vote. ANI methods are not mixed in one ranking: skani orders this table and any ANIm values are shown beside it.

| Tie group | Reference | Parent | skani ANI % | AF query % | AF reference % | ANIm identity QR/RQ % |
|---|---|---|---|---|---|---|
| 0 | primary_whole:GCA_013738375.1 | species ECTX-SP025 | 100.0 | 99.99 | 99.99 | — |
| 1 | primary_whole:GCA_001729745.1 | species ECTX-SP025 | 98.97 | 82.69 | 92.17 | — |
| 1 | primary_whole:GCA_013625435.1 | species ECTX-SP025 | 98.97 | 85.66 | 90.75 | — |
| 1 | primary_whole:GCA_052922205.1 | species ECTX-SP025 | 98.97 | 85.51 | 91.21 | — |
| 2 | primary_whole:GCA_027595125.1 | species ECTX-SP025 | 98.92 | 83.07 | 90.16 | — |

### Whole-panel species and lineage summary

| Parent | Panel references | Returned | Best skani ANI % | Among strongest matches | Challenging references |
|---|---|---|---|---|---|
| species ECTX-SP025 | 30 | 30 | 100.0 | yes | — |
| species ECTX-SP009 | 5 | 5 | 94.32 | no | — |
| species ECTX-SP018 | 1 | 1 | 93.76 | no | — |
| species ECTX-SP016 | 2 | 2 | 91.5 | no | — |
| species ECTX-SP003 | 8 | 8 | 89.67 | no | — |
| species ECTX-SP007 | 4 | 4 | 89.48 | no | — |
| species ECTX-SP019 | 14 | 14 | 89.29 | no | — |
| species ECTX-SP014 | 1 | 1 | 89.17 | no | — |
| species ECTX-SP013 | 8 | 8 | 89.14 | no | — |
| species ECTX-SP002 | 8 | 8 | 89.01 | no | — |
| species ECTX-SP017 | 4 | 4 | 88.7 | no | — |
| species ECTX-SP023 | 7 | 7 | 88.63 | no | — |
| species ECTX-SP015 | 1 | 1 | 88.62 | no | — |
| species ECTX-SP011 | 8 | 8 | 88.59 | no | — |
| species ECTX-SP024 | 3 | 3 | 88.56 | no | — |
| species ECTX-SP005 | 3 | 3 | 88.54 | no | — |
| species ECTX-SP010 | 1 | 1 | 88.48 | no | — |
| species ECTX-SP021 | 4 | 4 | 88.34 | no | — |
| species ECTX-SP008 | 13 | 13 | 88.33 | no | — |
| species ECTX-SP006 | 1 | 1 | 87.99 | no | — |
| species ECTX-SP001 | 1 | 1 | 87.77 | no | — |
| species ECTX-SP012 | 8 | 8 | 87.67 | no | — |
| species ECTX-SP004 | 3 | 3 | 87.55 | no | — |
| species ECTX-SP022 | 2 | 2 | 86.9 | no | — |

### Challenges

Not applicable: no rescue was attempted, so no challenge test ran.

### Supplementary results

- MLST: complete
  - state exact_profile_match, sequence type 286 (supplementary; it never sets a taxon)
- Genomovar search: assigned

### Provenance

- Query SHA-256: `603ed679732e01cb7d1a06ec73d89a53fc12b6f6b22f071df5dee866cdf982b9`
- Bases: 5215023; records: 8
- Evidence SHA-256: fa30effe40ead1906508371b3e110d4a4a982991c675e2122d7703767103b9b7

Data selection, citation and retained evidence:

    {
      "anim_evidence_directory": "single-alignments",
      "anim_manifest_sha256": "02cd57a36119558ae62b93d8d87b1ef5563f3c8009040fa0867da13027ad195d",
      "anim_trust": "embedded_catalogue",
      "catalogue_sha256": "873353f91dee10efc5f41ec86daceeeb5a6e841e97e885fdd7912853dda25ff9",
      "citation": {
        "doi": "10.6084/m9.figshare.34640883.v1",
        "record_version": 1,
        "title": "Enterobacter reference database for Pterobacter, version 2026-10-15",
        "url": "https://figshare.com/articles/dataset/Enterobacter_reference_database_for_Pterobacter_version_2026-10-15/34640883"
      },
      "database": "enterobacter-reference",
      "mlst_manifest_sha256": "0f0a4bd051e7a29d97f530a38982026df10ca223805e7b0147b19796763978dd",
      "mlst_requested": true,
      "mlst_trust": "embedded_catalogue",
      "reference_manifest_sha256": "f7c82d8e912fdb08aec93b190316f482fd462b3c514c5b374f8c694af1278e28",
      "reference_trust": "embedded_catalogue",
      "status": "published",
      "version": "2026-10-15"
    }

- Reference package 2026-10-15, manifest SHA-256 f7c82d8e912fdb08aec93b190316f482fd462b3c514c5b374f8c694af1278e28
- Software version: 0.0.0. Exact schema, method and analysis identities are retained in report.json.

### What to do next

The assignment is conditional on the approved rules and the supplied sequence. It does not establish that the isolate is pure.

