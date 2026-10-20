# Licensing and source availability

Modified combined package: Pterobacter. This distribution notice was added on 15 October 2026. The native-source notes and retained patches describe changes to upstream components.

The combined Pterobacter executable is distributed under [GPL-3.0-or-later](../LICENSE-GPL-3.0.txt). Pterobacter-authored source retains its [MIT licence](../LICENSE). These describe different scopes: the executable incorporates upstream code with additional conditions. The reference databases and MLST data have separate terms; this software licence does not replace them or apply automatically to your classification results.

You can run the program for any purpose. If you redistribute the combined executable, provide the applicable notices and complete corresponding source under the GPL conditions. Our releases must make that source available alongside the binaries. See [GPLv3 sections 1, 5 and 6](https://www.gnu.org/licenses/gpl-3.0.html) for the terms.

## Included components

MUMmer4's maintained source includes Artistic-2.0 code and GPL-3.0-or-later Jellyfish and skip-list headers. Jellyfish templates are compiled through the native bridge. The distribution uses the GPL-compatible option in Artistic-2.0 section 4(c)(ii), preserving the original notices and the documented maintenance patch. The [native-source README](../vendor/mummer4/README.md) records the upstream revision and changes. The [FSF licence list](https://www.gnu.org/licenses/license-list.html#ArtisticLicense2) describes this compatibility route.

[Third-party licences](../THIRD-PARTY-LICENSES.md) lists the Cargo and native notices to retain. MIT, Apache and other upstream grants remain attached to their source; the combined-executable licence does not erase them.

## Preparing a corresponding-source archive

This is a maintainer operation. It prepares files locally and does not publish a release.

From a clean committed checkout with the locked dependencies cached:

```sh
cargo fetch --locked
python3 scripts/prepare_source_archive.py \
  --staging-parent /path/to/temporary-storage \
  --output /path/outside/the/checkout/pterobacter-source.tar.gz
```

The script exports that exact commit, vendors locked registry sources offline into temporary storage, and adds relative Cargo source configuration. It rejects changed tracked files, missing notices, links and existing outputs. The archive contains source, build scripts, the Cargo lockfile, dependency sources, component notices and the MUMmer maintenance patch. `SOURCE-ARCHIVE.json` records the commit and every other file's size, mode and SHA-256. Its own digest and the archive digest are printed separately. Temporary vendoring is removed on exit and must not be committed.

Before distribution, extract the archive into a fresh directory, verify its complete inventory and build it with an empty Cargo home:

```sh
cd pterobacter-corresponding-source
CARGO_HOME=/path/to/empty-cargo-home \
CARGO_TARGET_DIR=/path/to/fresh-target \
  cargo install --path . --locked --offline \
  --root /path/to/fresh-install
```

A supported Rust toolchain and GNU C++ compiler must already be installed; they are not bundled. Run the installed synthetic smoke checks and record the actual target, compiler versions, binary hashes and source-archive hash. System libraries and compilers are separate from this source archive. If a binary package also ships runtime libraries, review those files and their terms explicitly.

Publish equivalent access to this exact corresponding source beside the executable download, with a clear link and matching release identity. GPLv3 section 6(d) allows a separate source download; users need not download it with the executable. Keep access available as required by the licence. The GitHub-generated source tarball alone does not include registry sources supplied by this preparation script.

The source archive is a software deliverable. It must not contain research workspaces, study genomes, validation outcomes, credentials or private material. Database publication and software release approval remain separate steps.
