# Release preparation

The reference database is public at [Figshare, version 2026-10-15](https://doi.org/10.6084/m9.figshare.34640883.v1). The first software release is still held for author approval. Source builds report version `0.0.0` until the release version is set.

## Prepared distribution channels

| Channel | Prepared and checked | Publication status |
|---|---|---|
| crates.io | One Cargo package; embedded engines; installation from the packaged source | First upload pending |
| Linux x86_64 | Precompiled commands, runtime-library manifest, notices and checksums | First release pending |
| Apple Silicon macOS | Precompiled commands; requires Homebrew GCC and XZ runtimes | First release pending |
| GHCR | Non-root Linux amd64 image with embedded ANIm and MLST | First push pending |
| Reference database | Published archives; pinned installer, component verification and MLST | Available |
| User guide | Built with mdBook and published on GitHub Pages | Available |

The Linux/macOS build, test and installed-command checks pass for the prepared candidate. An Apple Silicon Mac also passed the fixed scientific parity panel: 60 inputs, 120 component comparisons, 44 reciprocal ANIm pairs and 60 reports under each calling policy. This is platform parity for the tested panel, not a universal accuracy or compatibility claim. Linux ARM and Windows are untested.

[Real public-genome examples](examples/README.md) demonstrate the current output format and provide reproducible commands. They do not extend the independent validation dataset. The ANIm report retains its compiled `not_yet_validated` metadata; documentation does not change scientific status fields.

## Source and licences

The combined executable is distributed under GPL-3.0-or-later conditions. Pterobacter-authored source retains MIT. Complete corresponding source, locked dependencies and component notices accompany executable distribution. The source archive is checked by an offline rebuild. See [licensing](licensing.md).

Reference sketches, FASTAs and MLST have their own manifests and notices. The published export replaces the earlier private preparation package; it does not replace frozen research inputs. MLST is a 2026 retrieval of PubMLST submissions through 31 December 2024. See [database management](databases.md) and [MLST](historical-mlst.md).

## First-release sequence

1. Approve public source visibility and the first software release.
2. Review the release version, changelog and citation metadata. Update installation wording before tagging so the immutable crates.io README describes the intended channels.
3. Create the reviewed version tag and wait for successful main CI at that exact commit.
4. Run the Distribution workflow from main with that tag and publication enabled. It verifies the actual versioned crate, binaries, source archive and container before the publication job runs.
5. Verify public crate installation, binary downloads, checksums and a public-database example. Confirm GHCR package visibility and test an anonymous pull. Update the guide to describe the channels as available only after those checks pass.

Preparation artifacts named `0.0.0` must not be relabelled as a release. The release workflow builds the tagged version. A registry token is configured, but its publication permissions will only be exercised by the first approved upload.

The GHCR publication job uses `GITHUB_TOKEN` with `packages: write`, loads the exact tested image and publishes complete corresponding source first. It refuses to replace a different image under an existing version tag. For a publication-only network failure, rerun failed jobs in the original workflow run to reuse its verified artifacts. A fresh build of the same source is not guaranteed to be byte-identical.

Software publication, manuscript submission and formal taxonomic acts are separate approvals.
