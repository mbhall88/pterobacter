# MLST-UNGAPPED-r1

Issue #12 implements the MLST route chosen after issue #2. This contract names the new engine and describes the record it produces. Its job is to support the versioned amendment and pin mapping that the #24 integration needs before #25 evaluation. It does not amend P4a-r3, relabel the engine as `mlst`, or authorise evaluation or release.

## Engine identity

`terobacter-ungapped-mlst` version `1` is route D of the feasibility study. It finds exact 32-mer seeds between the allele set and the assembly, in both orientations, and extends each seeded diagonal without gaps. Scoring is +1/−2 with a raw X-drop of 13, and the first maximum is kept. Before the rules run, an HSP is kept only with at least 95% identity and a score that passes an e-value of 1e-20. Ambiguous bases and N are mismatches and never seed. Lower case is upper-cased. The code is Rust, compiled into `tero` and `terobacter`, and needs no external executable or alignment library.

The comparator is `mlst` 2.32.2 with BLAST+ 2.16.0, running `blastn -ungapped -word_size 32`. Terobacter does not run it. The comparator's rules were read from its GPL-2.0 source and observed in its output; no code was copied:

- a hit counts once its identical bases cover at least 50% of the allele;
- a full-length identical hit is exact; further exact hits append `,id`, and an exact hit replaces an earlier `~` or `?`;
- the first full-length inexact hit in report order gives `~id`, and the first shorter one gives `id?`;
- report order is contig, then the allele's best HSP score, ties going to the later allele in database order;
- the score is `int((7 − 0.3·tilde − 0.5·partial − missing) · 90/7)`, plus 10 with an ST, and the STATUS table follows from it.

The X-drop and tie rule were fitted on the development fixtures. They reproduce every comparator HSP and every call there, but that agreement is in-sample. BLAST's removal of HSPs with common endpoints and its query splitting are not modelled.

The record reports every value above under `engine.profile`, the comparator under `engine.comparator`, and the comparator-style ST, STATUS and SCORE under `comparator_style`. A receipt therefore names the engine that actually ran.

## Scheme package

The operator passes a package directory and the SHA-256 of its `manifest.json`. The loader applies the frozen importer's package checks. The manifest must be schema 1 for scheme `ecloacae`, with the seven loci in order, the 2024-12-31 submission cutoff, the exact cutoff limitation text, package ID, retrieval time, sequence state, data terms URL and `complete_through_cutoff_independently_established: false`. Each of the seven locus FASTAs and the profile table must match the SHA-256 listed in the manifest. Alleles must be `locus_N` records of A, C, G and T, with a digit ID that fits in `u64`. Larger IDs fail package validation before typing. STs and profiles must be unique, every profile must name alleles that exist, and the counts must match. The package is checked again before reports are written. Nothing is embedded in the binary, so whether to embed the scheme is still Michael's decision from #2.

## Record

The record keeps the frozen importer's semantics, with the same derivation:

- `state`: `exact_profile_match`, `unmatched_profile`, `incomplete_profile`, `inexact_alleles` or `ambiguous_alleles`. Only seven unique exact alleles that form a listed profile give `sequence_type`. An exact profile the scheme does not list is `unmatched_profile`, never a new ST.
- Per-locus `raw_token`, `state`, reported IDs and sequence-verified exact locations: record ID (the first header word), 1-based inclusive coordinates and strand. Every location is checked against the hashed query sequence.
- `profile_compatible_candidate_ST`, given only when every repeated locus has one allele at distinct locations, with `candidate_is_reported_ST: false` and the copy-diagnostic reasons.
- `database`: package ID, manifest SHA-256, data SHA-256s, cutoff, retrieval time, sequence state, limitation text, data terms URL, `complete_through_cutoff_independently_established: false` and `post_cutoff_currency: not_claimed`.
- `taxonomic_assignment_performed: false`. MLST never sets or changes an assignment.

Repeated record identifiers, which would make locations ambiguous, fail the record. A failure gives `status: failed`, the stage (`input`, `package`, `typing` or `package_recheck`) and the error. It never gives a negative result.

## Relation to the importer contract

The importer's prepared-command contract pins `mlst`, `blastn`, `any2fasta` and a BLAST index, and none of these exist here. This record replaces the importer's `execution` block with `engine` and drops `query_path` and the prepared command. The issue #2 diagnostic passed route D output to the importer under the comparator's command, and that receipt is not provenance for this engine. Mapping this engine onto the P4a-r3 executable and profile pins is the amendment that #24 must record before #25.

## Development evidence

`tests/mlst_dev_baseline.rs` matches all 52 valid development fixtures to the frozen comparator calls and importer records: tokens, ST, STATUS, SCORE, state, candidate ST, copy reasons and exact locations. `tests/fixtures/mlst-dev/README.md` records the derivation. Real-assembly parity is RG-4 in mbhall88/Enterobacter#25.
