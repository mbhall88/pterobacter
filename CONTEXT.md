# Pterobacter

Vocabulary for the Pterobacter tool, which classifies Enterobacter isolate assemblies. Terms from the research project (type reference, nearest-reference evidence, unresolved assignment) keep their research meaning here.

## Language

**Saved evidence**: A recorded set of skani comparisons between one query assembly and the primary reference panel, in the frozen 0.2.0 development-evidence format. Deciding from saved evidence runs no new comparison.

**Reference panel**: The references a query is compared against. Every returned row is a competitor, not a vote.

**Reference package**: The versioned set of files that holds the reference panel's data, listed with their lengths and SHA-256 digests. A package is authenticated before any decision uses it: its manifest must have the SHA-256 the operator expects, and every listed file must match the listing. Evidence is bound to the package by that same manifest SHA-256, and each reference it names must be one of the package's, with the taxa, names, name status and role the package gives it.

**Reference component**: One of the two components a reference package carries, `primary_whole` or `genomovar_chromosome`. Each has its own databases and build records. Species evidence comes from `primary_whole`.

**Classification rules**: The numerical gates that turn evidence into a call. Classification starts with skani and uses ANIm for eligible assemblies near the species boundary. `--no-anim` disables that alignment step.

**Trigger band**: 94.5 ≤ maximum returned skani ANI < 96.0 (with custom T, max(80, T − 1.5) ≤ ANI < T). Only queries in the band can be rescued.

**Rescue**: A species call supported by ANIm for a query in the trigger band. It needs a witness in the raw-nearest named species, a complete competitor panel and no challenge or veto. It never sets a subspecies.

**Witness candidate**: A reference in the raw-nearest named species with skani ANI in the trigger band and both skani AFs ≥80%. It becomes a rescue witness only if both ANIm identities exceed T and all four ANIm AFs are ≥80%.

**Competitor panel**: Every primary reference not masked out, including those skani did not return. A positive rescue needs a finished reciprocal ANIm pair for each one.

**Foreign-parent challenge**: Another parent with skani or either ANIm orientation at ≥ min(96, T) and reference AF ≥80%. It withholds a rescue whatever the query AF.

**Mask**: Reference IDs excluded for one evaluation setting, removed from the skani evidence, the panel counts and the competitor panel alike.

**Bound ANIm record**: A saved reciprocal ANIm pair record whose query hash, reference hash and method profile match the decision's inputs. Under the dry-plan rule it must also be a pair the plan requested.

**Dry rescue plan**: The trigger state, nearest parent, witness candidates and the ANIm pairs still required, computed from saved evidence. It runs no alignment.

**Decision**: The result of applying the classification rules to saved evidence. Species and subspecies are decided and reported separately.

**Species call**: Either assigned, with the species and its witness, or withheld with a reason. A withheld call is reported with status `unresolved`, the same thing the research project calls an unresolved assignment.

**Witness**: The reference that satisfies the ANI and aligned-fraction gates for the called species. It is evidence for the call, not the call itself. All of a witness's gates must pass on that one reference.

**Parent**: The taxon a reference belongs to for the purpose of finding competitors: its species, else its lineage, else the reference alone (reported as `untyped_reference`). Unmapped references are never merged into one parent.

**Genomovar**: An informal chromosome ANI group within subspecies `ECTX-SS007`, from the frozen definition `2026-10-02-v1` (25 groups, 173 historical chromosome members). A genomovar call is a conditional operational match to one group, not a population boundary or a new taxon.

**Genomovar parent**: The final species and subspecies call a genomovar depends on: actually assigned `ECTX-SP025` and `ECTX-SS007`, unvetoed, on the same query and package. The parent call must come from skani; an ANIm-supported species call does not establish the subspecies. Distinct from the competitor **Parent** above.

**Compatible group**: A genomovar group whose every member has reported skani ANI ≥99.50% to the query. A group with a missing member and no measured failure is incomplete, never excluded.

**Rejection witness**: A measured member below 99.50% that excludes its group, listed in the group's `below_threshold`. Only a rejection witness excludes a rival; missing evidence never does.

**Query label**: The assembly path as supplied, or as resolved from a list, that is read and passed to skani as the query's name. It is reported as `input.file` and never replaced by a sample name. Exact orientation-score ties use sketch content, so changing a filename does not select the measurement orientation.

**Sample name**: A user-supplied reporting name for one input, from `--sample-name` or a sample sheet. It labels reports only; it is never a query label, an output path or part of a decision.

**Front end**: Code that turns user input into evidence and a decision into output: the command line today, possibly a browser or GUI later. Front ends call the decision logic and never contain it.

**Development binary**: `pterobacter-dev`, built only with the `dev` feature, hosting evaluation-only operations. It shares the implementation with `ptero` and `pterobacter` but is not part of the user-facing tool.

**Method diagnostic**: A bound reciprocal ANIm comparison supplied alongside saved skani evidence. A disagreement at the species ANI/AF gate withholds the assignment. With boundary ANIm enabled, a witness candidate that fails skani only on ANI and passes the strict ANIm gate is an expected method crossing.

**Problem warning**: A bound external concern that withholds the assignment. An explained scope assessment cannot clear it.

**Historical MLST**: Optional supplementary seven-locus typing against the PubMLST *Enterobacter* scheme as submitted through 31 December 2024. It never sets or changes a species, subspecies, lineage or genomovar, and an exact profile the scheme lacks is unmatched, not a new ST.

**MLST scheme package**: The versioned, hash-listed allele FASTAs and profile table for historical MLST, authenticated by the SHA-256 of its manifest. It is separate from the reference package.

**MLST engine**: `terobacter-ungapped-mlst`, the in-binary matcher. It reproduces the comparator `mlst` 2.32.2 with BLAST+ 2.16.0 on development fixtures but is a different, named engine.
