# Preparing and publishing a release

The **Distribution** workflow prepares crates.io and precompiled downloads. Pull requests build private candidate artifacts; they do not publish. A manual run with **publish=false** also prepares artifacts without publication.

## What is built

- One `pterobacter-VERSION.crate`, containing the reviewed skani and partitions implementations as embedded modules, native ANIm sources and notices. There are no unpublished path dependencies or substituted upstream engines.
- Linux x86_64 and Apple Silicon macOS archives, each containing `ptero`, `pterobacter`, licences and a manifest with hashes, source identity and measured runtime libraries.
- A complete corresponding-source archive with the locked registry dependencies. CI rebuilds it offline with an empty Cargo home.
- `SHA256SUMS` for the release assets. Both platforms must produce the same Cargo archive. Binary and source manifests must identify the same commit.

Each binary is built by installing the extracted `.crate`, then tested with the installed-command smoke test and database/seeding regression fixtures. This checks the files users receive, rather than only the checkout. Scientific rules, reference bytes and database identities are unchanged.

Linux builds run on Ubuntu 22.04 and use glibc and GNU C++ runtime libraries. They are not static musl builds. macOS builds run on macOS 15 for Apple Silicon and require the Homebrew GCC runtime, installed with `brew install gcc`. The binary manifests record the actual linked libraries. Intel macOS, Linux ARM and Windows binaries are not part of this release matrix. Users need no Rust compiler or external analysis executable to run the binaries.

## Prepare a candidate

Merge the reviewed implementation and release-preparation work. Keep the release-please version PR separate until Michael approves the release. Run **Distribution** from main with the tag empty and publication disabled to inspect the `verified-release-assets` artifact.

For a local native check:

```sh
python3 scripts/build_binary_distribution.py \
  --target x86_64-unknown-linux-gnu --output /tmp/pterobacter-candidate
```

Use `aarch64-apple-darwin` on an Apple Silicon Mac. The output must be new and outside the checkout. Commit changes first; `--allow-dirty` exists only for local development and produces artifacts that the release collector refuses to publish.

## Publish an approved version

Publication makes source, crate and binaries public and requires Michael's approval. The source repository must be public for this workflow; changing its visibility is a separate decision. A configured token alone is not publication approval.

1. Review and merge the release-please version PR. Confirm the resulting version tag and GitHub release exist and main CI has passed for that exact commit.
2. Run **Distribution** from main, set the tag to the reviewed tag such as `v0.1.0`, and set **publish=true**. The workflow checks the tag against Cargo's version, its ancestry on main, repository visibility and successful source CI.
3. Both native builds and the corresponding-source rebuild must pass. The collector validates the inventories and hashes. The publisher then regenerates the crate and compares it byte-for-byte with the tested archive.
4. Only the publication job receives `CARGO_REGISTRY_TOKEN`. It uploads corresponding source before binaries, publishes the crate, verifies its registry checksum, then attaches the remaining assets.
5. Verify the public downloads and installation, then update the README and installation page to present the now-available channels. Until then, their availability remains described as pending.

If a network failure interrupts publication, rerun the same reviewed tag. Existing registry versions and GitHub assets are accepted only when their bytes match; differing content is refused, never replaced. Do not change a published version's contents to repair an upload.

The combined executable is GPL-3.0-or-later; Pterobacter-authored source retains MIT. Complete corresponding source and component notices accompany downloads. See [licensing](licensing.md). The `.crate` has registry dependencies; the separate corresponding-source archive supplies those source bytes for offline rebuilding.
