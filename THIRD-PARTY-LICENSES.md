# Third-party licences

The combined Pterobacter executable is distributed under [GPL-3.0-or-later](LICENSE-GPL-3.0.txt). Pterobacter-authored source keeps its [MIT licence](LICENSE); upstream components keep their own notices and grants. Ship both licence texts, this document, the generated [THIRDPARTY.json](THIRDPARTY.json) bundle and [THIRD-PARTY-NOTICES.txt](THIRD-PARTY-NOTICES.txt) with binaries, and provide the complete corresponding source described in [the source-distribution guide](docs/licensing.md). Only dependencies selected for a build are linked into its binary.

[cargo-bundle-licenses](https://github.com/sstadick/cargo-bundle-licenses) 4.2.0 collects licence texts for the locked runtime graph, including target-specific dependencies. The supplement retains source authors and repositories, transitive build dependency notices and source texts absent from the bundle. These include native code bundled with liblzma-sys, zstd-sys and tikv-jemalloc-sys, and miniprot's MIT notice for skani's DNA tables and hashing code. The two files replace the individual files formerly in `licenses/`.

skani 0.3.2 is vendored from `22da4ae9e44b2e3e686c8e5c71fcd16cec2b9791`, with its supplied MIT licence. partitions is vendored from `adf36eeaa4bb2b0042daa14cef28be36ac9be5da`, Apache-2.0. Both retain their original licence files. See [the fork notes](vendor/README.md) for modified files and compatibility semantics.

## Regeneration

The Cargo licence bundle does not cover the separately embedded MUMmer4 source. Ship `vendor/mummer4/LICENSE.md`, `GPL-3.0.txt`, `COPYRIGHT`, `ACKNOWLEDGEMENTS` and `README.md` with a binary distribution, together with the corresponding source and the maintenance patch. MUMmer4 includes Artistic-2.0 code and GPL-3.0-or-later Jellyfish/mt_skip_list headers. Jellyfish templates are compiled into the executable. The root MIT licence does not replace those terms. The combined-distribution route uses GPLv3 and Artistic-2.0's section 4(c)(ii) compatibility option; original notices and modification records remain intact. See the [native-source notes](vendor/mummer4/README.md) and [source-distribution guide](docs/licensing.md). Publication and final source-archive acceptance remain separate release gates.

Install the pinned tool once, then fetch the locked sources before generating notices:

```sh
cargo install cargo-bundle-licenses --version 4.2.0 --locked
cargo fetch --locked
just licenses
just licenses-check
```

Python 3.11 or later is required. The wrapper reads Cargo metadata and source archives offline, verifies that `Cargo.lock` stays unchanged and never copies source-cache paths into the outputs. JSON keeps generation and comparison within Python's standard library. Review both generated files when changing dependencies.

Some source archives omit licence files. For a missing text, add the reviewed text to the matching package's `licenses` entry in `THIRDPARTY.json`, then regenerate. The bundler's `--previous` option carries that text forward while the package name, version and licence match. A version update needs a fresh review. The initial bundle retains reviewed texts for cloudabi, enum-map, enum-map-derive, fxhash, nalgebra-macros, newtype_derive, r-efi and the two winapi target crates.

For an OR expression, the wrapper retains available licence options. Every constituent of an AND expression must have text. Missing required text stops generation. skani selects its supplied MIT option because the bundler also assigns that same text to its declared Apache alternative. The supplement preserves all other source notices, including alternatives omitted from the bundle.

CI installs the same pinned bundler, fetches all locked target sources and runs the check. It compares both complete files without rewriting them, so additions, removals and text edits all fail until the notices are regenerated and reviewed.

## A single notice bundle for binary packages

Packagers can generate one YAML document instead of shipping the separate Cargo,
supplemental and MUMmer notice files:

```sh
python3 -m pip install PyYAML
cargo fetch --locked
python3 scripts/bundle_licenses.py --distribution THIRDPARTY.yml
python3 scripts/bundle_licenses.py --check --distribution THIRDPARTY.yml
```

Ship `LICENSE`, `LICENSE-GPL-3.0.txt` and the generated `THIRDPARTY.yml`.
The YAML contains the Cargo texts, vendored skani and partitions licences,
supplemental source notices, and MUMmer licence files and source-comment notices.
All are read from the current locked sources; do not edit the YAML by hand.
This export leaves the two upstream generated files unchanged. PyYAML is required
only for this export. CI checks generation, coverage and stale-output rejection.

The source link names the corresponding-source archive for the package version.
Downstream distributors must also provide their patches and build scripts; this
notice bundle does not replace the corresponding-source requirement.
