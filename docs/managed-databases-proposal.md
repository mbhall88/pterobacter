# Managed databases: proposal for issue 39

This proposal awaits Astra's review together with the draft DC-1 distribution
contract from [Enterobacter #34](https://github.com/mbhall88/Enterobacter/issues/34#issuecomment-6185508536).
The commands below are proposed interfaces. They are **not implemented commands**.
The current CLI still uses explicit package paths and digests; see [assembly.md](assembly.md).

## Proposed workflow

```text
ptero database install VERSION
ptero database list
ptero database verify VERSION
ptero --database VERSION assembly.fna
ptero --database VERSION --mlst assembly.fna
```

`-D/--database` selects an exact release. `-B/--database-dir` selects a store instead
of the platform data directory. A reviewed catalogue may pin one exact default;
neither directory order, modification time nor the number of installations selects
one. There is no `latest` alias. Missing or invalid selected versions fail without
using another version. Classification stays offline and never provisions data.

The public catalogue should be shipped with the software and pin component
versions, byte lengths, archive SHA-256, exhaustive member inventories and
scientific manifest SHA-256 independently of the payload. Its exact serialization,
compatibility fields, citation fields and authoritative location require the #34
milestone and Astra's acceptance. A custom catalogue needs an explicit trusted
source and digest; a checksum inside an untrusted download is insufficient.
No published download URL or DOI is assigned by this implementation milestone.
Local fixture sources and any eventual published URLs must remain distinguishable.

## Explicit packages and optional components

`--reference-package` and `--expected-sha256` remain a pair and conflict with an
explicit managed release selection. Explicit ANIm and MLST path/digest pairs
replace only their respective managed components, after existing verification and
binding checks. A failure never causes a fallback to a managed or other version.

Managed classification resolves the matching sketch/index and complete primary
whole-genome FASTA components. Boundary ANIm retains its current scientific and
execution settings; [issue 43](https://github.com/mbhall88/pterobacter/issues/43)
owns execution-limit changes. Assembly input options remain with
[issue 40](https://github.com/mbhall88/pterobacter/issues/40).

`-l/--mlst` is opt-in. An unavailable optional scheme must yield an explicit typing
failure without altering species, subspecies or genomovar calls. Reports must name
its package, manifest digest, retrieval date, source and currency limitation.
Historical schema 1 keeps its submission cutoff and exact limitation text.

A separately versioned current/custom scheme contract should carry the supported
seven-locus order, independently pinned data files, package identity, retrieval
instant, source URLs, sequence state, submission coverage or an explicit unknown
coverage statement, and source terms with a recorded rights disposition. A recent
retrieval instant alone cannot establish database currency. A custom source must
not be described as PubMLST without source evidence. New metadata must be reported
from that manifest; it must not inherit schema 1's historical date or claims.
Loader adaptation and synthetic complete/incomplete/ambiguous typing tests follow
contract acceptance, before any new scheme becomes publicly supported.

## Installer safety layer implemented in this milestone

`src/cli/database/` provides transport primitives, exact-version selection and
unique retained evidence reservations. No public command, catalogue parser, HTTP
adapter or scientific loader compatibility promise is wired into the CLI yet.
The caller supplies pins and a scientific-verification callback. Production
integration must use the existing reference, ANIm and MLST verifiers and verify
all cross-component relationships before publication or reuse.

Each transport component is a single-member gzip-compressed portable tar with
regular files and optional expected directory entries. An explicitly pinned
top-level directory may be stripped after path validation; siblings are refused. The installer verifies the compressed
byte count and SHA-256 before extraction, then enforces the exact file inventory,
lengths and digests. It rejects unsafe paths, duplicate members, links, devices,
FIFOs, sparse members, GNU/PAX extensions, unknown directories/files, bad gzip
trailers, missing tar terminators and decoded streams beyond a trusted ceiling.
It never installs archive permissions, ownership or executable bits. The archive
profile is a proposed transport restriction for #34 review, not an accepted
public release format. Paths are UTF-8, at most 255 bytes, with forward slash
separators and no empty, dot, dot-dot, colon, backslash or control components.

A private owned store, exclusive per-version lock and same-filesystem staging
protect installation. Files and directories are synced before an atomic
no-replace rename. Existing releases are verified and reused offline. Corruption,
scientific incompatibility or lack of space stops the install without replacing
an earlier release. Failed attempts remove only their own staging and lock;
process death leaves both for inspection. Existing locks are never guessed stale.
Space preflight uses filesystem availability; quota exhaustion can still happen
later and is an error. Locks/stages left by a dead process require explicit
owner inspection and cleanup; no automatic cleanup command is supplied here.

Linux uses `renameat2` with `RENAME_NOREPLACE`; macOS uses `renamex_np` with
`RENAME_EXCL`. A filesystem that lacks this operation is refused. The worker's
shared job filesystem returned `EINVAL`; local `/tmp` supported the operation.
There is no fallback that weakens no-clobber behavior. Astra should decide whether
an alternative on-disk publication layout is needed for those filesystems before
freezing the format. A local supported filesystem remains available for testing.

Evidence reservations create a new private directory for each run and retain
it. A future CLI adapter must choose the platform state root, report the actual
evidence location, avoid creating output on invalid arguments, and respect
explicit output destinations. Concurrent reservations never reuse a directory.

## Optional current scheme disposition

Checked 15 October 2026: the [official Enterobacter page](https://pubmlst.org/organisms/enterobacter-spp)
identifies its scheme and citation, and the [data page](https://pubmlst.org/data)
links allele/profile downloads and recommends the API. These pages do not establish
that an anonymous download contains post-2024 submissions.

The [current PubMLST terms](https://pubmlst.org/terms-conditions), updated 13 May
2026, distinguish pre-2025 submissions from later submissions. They permit
redistribution of the former with citation and acknowledgement, restrict access
and use of the latter, and prohibit redistribution of significant portions of
post-2024 downloaded data. The [change record](https://pubmlst.org/changes-terms-and-conditions)
also covers tools that facilitate access to restricted data.

Disposition for Astra: historical provisioning remains separately reviewable;
current PubMLST provisioning is unavailable pending a source-specific rights and
access disposition. No new allele/profile data, credentials or account actions
were requested, and no curator was contacted. Synthetic current/custom packages
can test software independently of those rights. A working public current-scheme
download example cannot be claimed from the evidence above.

## Review and remaining acceptance

The safety tests exercise corrupted and truncated streams, path/link attacks,
missing/extra/hash-mismatched contents, scientific-verifier rejection, per-version
concurrency, space errors, ordinary cancellation, process death, offline reuse,
exact selection and retained evidence. They use generated transport bytes, not
new biological results. An integration test archives the existing synthetic reference, ANIm and historical
MLST derivations, verifies them with the existing scientific loaders, compares
the entire installed explicit-path classification report with the original, and
rejects a transport-authenticated ANIm component bound to another reference.
This proves local transport preservation, not the still-pending public export.
Existing CLI tests remain the regression gates.

Astra must review the CLI and catalogue proposal against the draft #34 DC-1 before its format
is frozen. The catalogue needs either exhaustive member pins or a specified,
authenticated derivation of that inventory. Interrupted-stage cleanup, per-version
versus per-component activation, and filesystems lacking atomic no-replace rename
remain concrete coordination questions posted on #34. Remaining work is the accepted catalogue adapter and compatible
component bindings, downloader and commands, platform path resolution, public
classification/MLST integration, current/custom scheme loader, task-oriented help
and local examples, accepted real #34 export integration and explicit-path
scientific-field parity. Enterobacter #35 separately verifies a published version.
Passing transport tests does not complete issue 39, its parent #30 or a release.
