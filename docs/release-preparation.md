# Private release preparation

**Current distribution policy:** the combined executable will be distributed under GPL-3.0-or-later conditions, while Pterobacter-authored source retains MIT. Complete corresponding source and component notices accompany executable distribution; see [licensing and source availability](licensing.md). Publication, final archive verification and platform acceptance remain separate. The measurements below describe the earlier evaluated baseline and are not current release sizes or dependency counts.

The evaluated baseline is `834f4c288e9c163f316f3668f0219004d0adc6da`. Implementation and its frozen evaluation are coordinator-accepted. The software still reports `0.0.0`; the reference data has its separate `2026-10-06.proposal1` identity. The source commit, binary and data-manifest digests, analysis settings and method profile identify a candidate more precisely than the placeholder software version.

## Measured installation and platform scope

An independent reviewer installed the exact baseline from a source export into a fresh Linux target/install directory using the locked dependency cache. It exposed only `tero` and `terobacter`; both passed installed synthetic skani-only and ANIm smoke tests without an external analysis executable on PATH. The source export contained 353 files, 7,822,152 file bytes, with no symlinks or matched private/research-workspace paths. The baseline source archive SHA-256 was `7f7ea1bb9453f6af71be4c6936a471052b3fde39e3d61888b8abaee6992a875c`.

| Item | Measured result |
|---|---|
| Linux binaries | 5,570,408 bytes each; `tero` SHA-256 `8f8ea3d9c0c3df68b7392aab34187942cc4e31b82605f7326e27764b256dadc5`; `terobacter` SHA-256 `f063d70ac94e0b7854ceeb31027ffc96314f24a931f92476284fb9ba9b4960f3` |
| Individual gzip-9 binary sizes | 2,141,021 bytes for `tero`; 2,141,104 bytes for `terobacter` |
| Linux runtime libraries | libstdc++, libgcc_s, libm, libc and the ELF loader; maximum inspected glibc symbol version 2.34 |
| Reference package file bytes | 176,905,716 across 46 files |
| Authenticated primary ANIm FASTA component | 696,137,782 bytes across 142 files |
| Historical MLST package file bytes | 2,568,699 across 29 files, including provenance/terms; 4,751 alleles and 3,291 profiles |
| Private tar.gz payload sizes | Reference 122,154,116 bytes; primary FASTAs 210,741,944 bytes; historical MLST 173,634 bytes; Linux binaries/notices 4,460,497 bytes. Gzip level 6, with complete member/hash readback |
| Linux scientific platform baseline | 60 fixed native records, 120 component comparisons and 44 reciprocal ANIm pairs with 88 orientations, retained from the accepted evaluation |
| macOS | CI build/test and synthetic installed smoke passed for the baseline. Final arm64 scientific EN-3 execution pending; no numerical platform acceptance yet |
| Linux ARM / Windows | Untested; native build currently requires Linux or macOS process containment |

These are observed cluster/build sizes and checks, not measured laptop timings or universal compatibility claims. The binaries contain Rust and embedded C/C++. Optional MLST has no extra executable dependency. Optional manual-tree tools are separate and are not included in these runtime sizes.

## Reference identity and rebuild limits

The authenticated reference manifest SHA-256 is `5743c25bcd706242ff8a28863fd6af2bf60e76f4c6af8b50d3f77deaf2e01dae`. The primary FASTA component and optional MLST each have a distinct manifest digest. The [coverage table](supported-taxa.md) preserves supported versus unsupported groups, type exceptions, name qualifications and component scopes. No new reference or renamed database index is substituted in this preparation.

All expected frozen package/database hashes are verified. Both indexes were independently reconstructed from all decoded existing sketch records: 140 primary entries and 173 chromosome entries reproduced the exact index SHA-256s. Re-encoded sketch payload bytes differed, so those diagnostic payloads are retained separately and are never substituted into the frozen package. This checks index names, offsets and lengths; it is not a source-FASTA re-sketching result. Archived source labels remain opaque index identities. Historical v1 skani measurements used query and reference labels to break orientation ties; the new content-order v2 profile removes that dependence and requires separate re-evaluation. A relabelled or sanitised reference release therefore needs an explicit new manifest and affected parity checks.

The current data package retains development-only, not-distributable metadata and private research provenance paths. It is provisioned privately for verification, not offered as a public data download. A separately reviewed export, reference-version designation and redistribution record remain author/coordinator release decisions. The frozen data stays intact.

## Licences and attribution

Pterobacter's own code is MIT. Preserve the [Cargo and native component notices](../THIRD-PARTY-LICENSES.md) in every prepared binary package. The locked Cargo bundle contains 201 crates; supplemental attribution covers 212 source packages and was independently reproduced. The embedded MUMmer4 subset also includes Artistic-2.0 and GPL-3.0-or-later notices. The root MIT licence is not a blanket replacement for component terms. The approved combined-distribution route and remaining archive checks are described above.

The historical MLST data is a 2026 retrieval of submissions through 31 December 2024, not a 2024 byte snapshot. Pre-2025 data may be redistributed with scientific citation and acknowledgement under [PubMLST's terms](https://pubmlst.org/terms-conditions). The package makes no claim of complete historical coverage or post-2024 currency. Cite the PubMLST resource and the relevant scheme when using it.

NCBI-derived assemblies and sketches retain the [NCBI molecular-data disclaimer](https://www.ncbi.nlm.nih.gov/About/disclaimer.html): NCBI does not transfer submitter rights or provide unrestricted third-party permission. The exact assembly accessions, byte digests and source provenance remain recorded. No curator or maintainer contact was made for this preparation.

## Review before publication

Private archives, manifests, platform instructions and evidence are prepared for review. Public repository visibility, uploads/releases, reference replacements, final software/data versions and author approval remain separate. The frozen ANIm report keeps `not_yet_validated`; acceptance of the evaluation does not silently rewrite that compiled status or imply independent accuracy. Changes to that status need a reviewed candidate and affected validation.

README wording, installation channels, examples, help and option naming can now receive Michael's repository presentation review. Cosmetic feedback does not authorize changes to thresholds, measurement engines, references or classification rules. Each request will receive a checked change or an agreed disposition.
