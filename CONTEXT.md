# Terobacter

Vocabulary for the Terobacter tool, which classifies Enterobacter isolate assemblies. Terms from the research project (type reference, nearest-reference evidence, unresolved assignment) keep their research meaning here.

## Language

**Saved evidence**: A recorded set of skani comparisons between one query assembly and the primary reference panel, in the frozen 0.2.0 development-evidence format. Deciding from saved evidence runs no new comparison.

**Reference panel**: The references a query is compared against. Every returned row is a competitor, not a vote.

**Reference package**: The versioned set of files that holds the reference panel's data, listed with their lengths and SHA-256 digests. A package is authenticated before any decision uses it: its manifest must have the SHA-256 the operator expects, and every listed file must match the listing. Evidence is bound to the package by that same manifest SHA-256.

**Reference component**: One of the two components a reference package carries, `primary_whole` or `genomovar_chromosome`. Each has its own databases and build records. Species evidence comes from `primary_whole`.

**Calling policy**: The numerical gates and rules that turn evidence into a call. v1 is `conservative-assembly-v1`, frozen from the 0.2.0 release.

**Decision**: The result of applying a calling policy to saved evidence. Species and subspecies are decided and reported separately.

**Species call**: Either assigned, with the species and its witness, or withheld with a reason. A withheld call is reported with status `unresolved`, the same thing the research project calls an unresolved assignment.

**Witness**: The reference that satisfies the ANI and aligned-fraction gates for the called species. It is evidence for the call, not the call itself.

**Parent**: The taxon a reference belongs to for the purpose of finding competitors: its species, else its lineage, else the reference alone. Unmapped references are never merged into one parent.

**Front end**: Code that turns user input into evidence and a decision into output: the command line today, possibly a browser or GUI later. Front ends call the decision logic and never contain it.

**Development binary**: `terobacter-dev`, built only with the `dev` feature, hosting evaluation-only operations. It shares the implementation with `tero` and `terobacter` but is not part of the user-facing tool.

**Method diagnostic**: A bound reciprocal ANIm comparison supplied alongside saved skani evidence. Under v1, a disagreement at the species ANI/AF gate withholds the assignment. It cannot rescue one.

**Problem warning**: A bound external concern that withholds the assignment. An explained scope assessment cannot clear it.
