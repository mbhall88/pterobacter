# Third-party licences

The locked runtime dependency graph, including target-specific dependencies, is listed below. Each notice retains the licence and copyright text shipped with its source package. For packages that declare Apache-2.0 but ship no separate licence file, the notice includes their declared authors and the Apache-2.0 text. Only dependencies selected for a build are linked into its binary.

skani is vendored from `22da4ae9e44b2e3e686c8e5c71fcd16cec2b9791` under its supplied MIT licence. Its modified files and compatibility semantics are documented in [the fork notes](vendor/README.md). The vendored partitions revision is `adf36eeaa4bb2b0042daa14cef28be36ac9be5da`, Apache-2.0. DNA tables and hashing code in skani also retain [miniprot's MIT notice](licenses/miniprot.txt).

The lzma-sys, bzip2-sys and tikv-jemalloc-sys notices include the bundled native-code licences. Redistribution of binaries must include this file and the `licenses/` directory, along with Terobacter's own `LICENSE`.

Regenerate after changing `Cargo.lock` with `python3 scripts/generate_license_notices.py`. The script reads the local Cargo source cache and never copies source-cache paths into notices.

| Dependency | Version | Declared licence | Text |
|---|---|---|---|
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | [notice](licenses/adler2-2.0.1.txt) |
| aho-corasick | 1.1.5 | Unlicense OR MIT | [notice](licenses/aho-corasick-1.1.5.txt) |
| anstream | 1.0.0 | MIT OR Apache-2.0 | [notice](licenses/anstream-1.0.0.txt) |
| anstyle | 1.0.14 | MIT OR Apache-2.0 | [notice](licenses/anstyle-1.0.14.txt) |
| anstyle-parse | 1.0.0 | MIT OR Apache-2.0 | [notice](licenses/anstyle-parse-1.0.0.txt) |
| anstyle-query | 1.1.5 | MIT OR Apache-2.0 | [notice](licenses/anstyle-query-1.1.5.txt) |
| anstyle-wincon | 3.0.11 | MIT OR Apache-2.0 | [notice](licenses/anstyle-wincon-3.0.11.txt) |
| anyhow | 1.0.104 | MIT OR Apache-2.0 | [notice](licenses/anyhow-1.0.104.txt) |
| approx | 0.5.1 | Apache-2.0 | [notice](licenses/approx-0.5.1.txt) |
| atty | 0.2.14 | MIT | [notice](licenses/atty-0.2.14.txt) |
| autocfg | 0.1.8 | Apache-2.0 OR MIT | [notice](licenses/autocfg-0.1.8.txt) |
| autocfg | 1.5.1 | Apache-2.0 OR MIT | [notice](licenses/autocfg-1.5.1.txt) |
| bincode | 1.3.3 | MIT | [notice](licenses/bincode-1.3.3.txt) |
| bio | 1.4.0 | MIT | [notice](licenses/bio-1.4.0.txt) |
| bio-types | 1.0.4 | MIT | [notice](licenses/bio-types-1.0.4.txt) |
| bit-set | 0.5.3 | MIT/Apache-2.0 | [notice](licenses/bit-set-0.5.3.txt) |
| bit-vec | 0.5.1 | MIT/Apache-2.0 | [notice](licenses/bit-vec-0.5.1.txt) |
| bit-vec | 0.6.3 | MIT/Apache-2.0 | [notice](licenses/bit-vec-0.6.3.txt) |
| bitflags | 1.3.2 | MIT/Apache-2.0 | [notice](licenses/bitflags-1.3.2.txt) |
| bitflags | 2.13.2 | MIT OR Apache-2.0 | [notice](licenses/bitflags-2.13.2.txt) |
| block-buffer | 0.10.4 | MIT OR Apache-2.0 | [notice](licenses/block-buffer-0.10.4.txt) |
| buffer-redux | 1.1.0 | MIT OR Apache-2.0 | [notice](licenses/buffer-redux-1.1.0.txt) |
| bv | 0.11.1 | MIT/Apache-2.0 | [notice](licenses/bv-0.11.1.txt) |
| bytecount | 0.6.9 | Apache-2.0/MIT | [notice](licenses/bytecount-0.6.9.txt) |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT | [notice](licenses/bytemuck-1.25.2.txt) |
| byteorder | 1.5.0 | Unlicense OR MIT | [notice](licenses/byteorder-1.5.0.txt) |
| bzip2 | 0.4.4 | MIT/Apache-2.0 | [notice](licenses/bzip2-0.4.4.txt) |
| bzip2-sys | 0.1.11+1.0.8 | MIT/Apache-2.0 | [notice](licenses/bzip2-sys-0.1.11+1.0.8.txt) |
| cc | 1.6.0 | MIT OR Apache-2.0 | [notice](licenses/cc-1.6.0.txt) |
| cfg-if | 0.1.10 | MIT/Apache-2.0 | [notice](licenses/cfg-if-0.1.10.txt) |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | [notice](licenses/cfg-if-1.0.5.txt) |
| clap | 3.2.25 | MIT OR Apache-2.0 | [notice](licenses/clap-3.2.25.txt) |
| clap | 4.6.7 | MIT OR Apache-2.0 | [notice](licenses/clap-4.6.7.txt) |
| clap_builder | 4.6.7 | MIT OR Apache-2.0 | [notice](licenses/clap_builder-4.6.7.txt) |
| clap_derive | 3.2.25 | MIT OR Apache-2.0 | [notice](licenses/clap_derive-3.2.25.txt) |
| clap_derive | 4.6.7 | MIT OR Apache-2.0 | [notice](licenses/clap_derive-4.6.7.txt) |
| clap_lex | 0.2.4 | MIT OR Apache-2.0 | [notice](licenses/clap_lex-0.2.4.txt) |
| clap_lex | 1.1.1 | MIT OR Apache-2.0 | [notice](licenses/clap_lex-1.1.1.txt) |
| cloudabi | 0.0.3 | BSD-2-Clause | [notice](licenses/cloudabi-0.0.3.txt) |
| colorchoice | 1.0.5 | MIT OR Apache-2.0 | [notice](licenses/colorchoice-1.0.5.txt) |
| cpufeatures | 0.2.17 | MIT OR Apache-2.0 | [notice](licenses/cpufeatures-0.2.17.txt) |
| crc32fast | 1.5.2 | MIT OR Apache-2.0 | [notice](licenses/crc32fast-1.5.2.txt) |
| crossbeam-deque | 0.8.8 | MIT OR Apache-2.0 | [notice](licenses/crossbeam-deque-0.8.8.txt) |
| crossbeam-epoch | 0.9.21 | MIT OR Apache-2.0 | [notice](licenses/crossbeam-epoch-0.9.21.txt) |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 | [notice](licenses/crossbeam-utils-0.8.23.txt) |
| crypto-common | 0.1.7 | MIT OR Apache-2.0 | [notice](licenses/crypto-common-0.1.7.txt) |
| csv | 1.4.0 | Unlicense/MIT | [notice](licenses/csv-1.4.0.txt) |
| csv-core | 0.1.13 | Unlicense/MIT | [notice](licenses/csv-core-0.1.13.txt) |
| custom_derive | 0.1.7 | MIT/Apache-2.0 | [notice](licenses/custom_derive-0.1.7.txt) |
| derive-new | 0.6.0 | MIT | [notice](licenses/derive-new-0.6.0.txt) |
| digest | 0.10.7 | MIT OR Apache-2.0 | [notice](licenses/digest-0.10.7.txt) |
| editdistancek | 1.0.2 | MIT | [notice](licenses/editdistancek-1.0.2.txt) |
| either | 1.19.0 | MIT OR Apache-2.0 | [notice](licenses/either-1.19.0.txt) |
| enum-map | 1.1.1 | MIT/Apache-2.0 | [notice](licenses/enum-map-1.1.1.txt) |
| enum-map-derive | 0.6.0 | MIT/Apache-2.0 | [notice](licenses/enum-map-derive-0.6.0.txt) |
| equivalent | 1.0.2 | Apache-2.0 OR MIT | [notice](licenses/equivalent-1.0.2.txt) |
| errno | 0.3.14 | MIT OR Apache-2.0 | [notice](licenses/errno-0.3.14.txt) |
| fastrand | 1.9.0 | Apache-2.0 OR MIT | [notice](licenses/fastrand-1.9.0.txt) |
| fastrand | 2.5.0 | Apache-2.0 OR MIT | [notice](licenses/fastrand-2.5.0.txt) |
| feature-probe | 0.1.1 | MIT/Apache-2.0 | [notice](licenses/feature-probe-0.1.1.txt) |
| find-msvc-tools | 0.1.14 | MIT OR Apache-2.0 | [notice](licenses/find-msvc-tools-0.1.14.txt) |
| fixedbitset | 0.4.2 | MIT/Apache-2.0 | [notice](licenses/fixedbitset-0.4.2.txt) |
| flate2 | 1.1.10 | MIT OR Apache-2.0 | [notice](licenses/flate2-1.1.10.txt) |
| fnv | 1.0.7 | Apache-2.0 / MIT | [notice](licenses/fnv-1.0.7.txt) |
| fuchsia-cprng | 0.1.1 | None | [notice](licenses/fuchsia-cprng-0.1.1.txt) |
| fxhash | 0.2.1 | Apache-2.0/MIT | [notice](licenses/fxhash-0.2.1.txt) |
| gbdt | 0.1.1 | Apache-2.0 | [notice](licenses/gbdt-0.1.1.txt) |
| gcollections | 1.5.0 | MIT/Apache-2.0 | [notice](licenses/gcollections-1.5.0.txt) |
| generic-array | 0.14.7 | MIT | [notice](licenses/generic-array-0.14.7.txt) |
| getrandom | 0.2.17 | MIT OR Apache-2.0 | [notice](licenses/getrandom-0.2.17.txt) |
| getrandom | 0.4.3 | MIT OR Apache-2.0 | [notice](licenses/getrandom-0.4.3.txt) |
| getset | 0.1.2 | MIT | [notice](licenses/getset-0.1.2.txt) |
| hashbrown | 0.12.3 | MIT OR Apache-2.0 | [notice](licenses/hashbrown-0.12.3.txt) |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 | [notice](licenses/hashbrown-0.17.1.txt) |
| heck | 0.4.1 | MIT OR Apache-2.0 | [notice](licenses/heck-0.4.1.txt) |
| heck | 0.5.0 | MIT OR Apache-2.0 | [notice](licenses/heck-0.5.0.txt) |
| hermit-abi | 0.1.19 | MIT/Apache-2.0 | [notice](licenses/hermit-abi-0.1.19.txt) |
| indexmap | 1.9.3 | Apache-2.0 OR MIT | [notice](licenses/indexmap-1.9.3.txt) |
| indexmap | 2.14.2 | Apache-2.0 OR MIT | [notice](licenses/indexmap-2.14.2.txt) |
| instant | 0.1.12 | BSD-3-Clause | [notice](licenses/instant-0.1.12.txt) |
| intervallum | 1.4.0 | MIT/Apache-2.0 | [notice](licenses/intervallum-1.4.0.txt) |
| is_terminal_polyfill | 1.70.2 | MIT OR Apache-2.0 | [notice](licenses/is_terminal_polyfill-1.70.2.txt) |
| itertools | 0.11.0 | MIT OR Apache-2.0 | [notice](licenses/itertools-0.11.0.txt) |
| itertools-num | 0.1.3 | MIT/Apache-2.0 | [notice](licenses/itertools-num-0.1.3.txt) |
| itoa | 1.0.18 | MIT OR Apache-2.0 | [notice](licenses/itoa-1.0.18.txt) |
| lazy_static | 1.5.1 | MIT OR Apache-2.0 | [notice](licenses/lazy_static-1.5.1.txt) |
| libc | 0.2.190 | MIT OR Apache-2.0 | [notice](licenses/libc-0.2.190.txt) |
| libm | 0.2.16 | MIT | [notice](licenses/libm-0.2.16.txt) |
| linux-raw-sys | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | [notice](licenses/linux-raw-sys-0.12.1.txt) |
| log | 0.4.34 | MIT OR Apache-2.0 | [notice](licenses/log-0.4.34.txt) |
| lzma-sys | 0.1.20 | MIT/Apache-2.0 | [notice](licenses/lzma-sys-0.1.20.txt) |
| matrixmultiply | 0.3.11 | MIT/Apache-2.0 | [notice](licenses/matrixmultiply-0.3.11.txt) |
| memchr | 2.8.3 | Unlicense OR MIT | [notice](licenses/memchr-2.8.3.txt) |
| memmap2 | 0.9.11 | MIT OR Apache-2.0 | [notice](licenses/memmap2-0.9.11.txt) |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 | [notice](licenses/miniz_oxide-0.9.1.txt) |
| multimap | 0.8.3 | MIT/Apache-2.0 | [notice](licenses/multimap-0.8.3.txt) |
| nalgebra | 0.29.0 | BSD-3-Clause | [notice](licenses/nalgebra-0.29.0.txt) |
| nalgebra | 0.33.3 | Apache-2.0 | [notice](licenses/nalgebra-0.33.3.txt) |
| nalgebra-macros | 0.1.0 | Apache-2.0 | [notice](licenses/nalgebra-macros-0.1.0.txt) |
| ndarray | 0.15.6 | MIT OR Apache-2.0 | [notice](licenses/ndarray-0.15.6.txt) |
| needletail | 0.5.1 | MIT | [notice](licenses/needletail-0.5.1.txt) |
| newtype_derive | 0.1.6 | MIT/Apache-2.0 | [notice](licenses/newtype_derive-0.1.6.txt) |
| num-bigint | 0.4.8 | MIT OR Apache-2.0 | [notice](licenses/num-bigint-0.4.8.txt) |
| num-complex | 0.4.6 | MIT OR Apache-2.0 | [notice](licenses/num-complex-0.4.6.txt) |
| num-integer | 0.1.47 | MIT OR Apache-2.0 | [notice](licenses/num-integer-0.1.47.txt) |
| num-rational | 0.4.2 | MIT OR Apache-2.0 | [notice](licenses/num-rational-0.4.2.txt) |
| num-traits | 0.2.19 | MIT OR Apache-2.0 | [notice](licenses/num-traits-0.2.19.txt) |
| once_cell | 1.21.4 | MIT OR Apache-2.0 | [notice](licenses/once_cell-1.21.4.txt) |
| once_cell_polyfill | 1.70.2 | MIT OR Apache-2.0 | [notice](licenses/once_cell_polyfill-1.70.2.txt) |
| ordered-float | 3.9.1 | MIT | [notice](licenses/ordered-float-3.9.1.txt) |
| os_str_bytes | 6.5.1 | MIT OR Apache-2.0 | [notice](licenses/os_str_bytes-6.5.1.txt) |
| partitions | 0.2.4 | Apache-2.0 | [notice](licenses/partitions-0.2.4.txt) |
| paste | 1.0.15 | MIT OR Apache-2.0 | [notice](licenses/paste-1.0.15.txt) |
| petgraph | 0.6.4 | MIT OR Apache-2.0 | [notice](licenses/petgraph-0.6.4.txt) |
| pkg-config | 0.3.34 | MIT OR Apache-2.0 | [notice](licenses/pkg-config-0.3.34.txt) |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 | [notice](licenses/ppv-lite86-0.2.21.txt) |
| proc-macro-error | 1.0.4 | MIT OR Apache-2.0 | [notice](licenses/proc-macro-error-1.0.4.txt) |
| proc-macro-error-attr | 1.0.4 | MIT OR Apache-2.0 | [notice](licenses/proc-macro-error-attr-1.0.4.txt) |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | [notice](licenses/proc-macro2-1.0.107.txt) |
| proptest | 0.8.7 | MIT/Apache-2.0 | [notice](licenses/proptest-0.8.7.txt) |
| quick-error | 1.2.3 | MIT/Apache-2.0 | [notice](licenses/quick-error-1.2.3.txt) |
| quote | 1.0.47 | MIT OR Apache-2.0 | [notice](licenses/quote-1.0.47.txt) |
| r-efi | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | [notice](licenses/r-efi-6.0.0.txt) |
| rand | 0.5.6 | MIT/Apache-2.0 | [notice](licenses/rand-0.5.6.txt) |
| rand | 0.6.5 | MIT/Apache-2.0 | [notice](licenses/rand-0.6.5.txt) |
| rand | 0.8.8 | MIT OR Apache-2.0 | [notice](licenses/rand-0.8.8.txt) |
| rand_chacha | 0.1.1 | MIT/Apache-2.0 | [notice](licenses/rand_chacha-0.1.1.txt) |
| rand_chacha | 0.3.1 | MIT OR Apache-2.0 | [notice](licenses/rand_chacha-0.3.1.txt) |
| rand_core | 0.3.1 | MIT/Apache-2.0 | [notice](licenses/rand_core-0.3.1.txt) |
| rand_core | 0.4.2 | MIT/Apache-2.0 | [notice](licenses/rand_core-0.4.2.txt) |
| rand_core | 0.6.4 | MIT OR Apache-2.0 | [notice](licenses/rand_core-0.6.4.txt) |
| rand_distr | 0.4.3 | MIT OR Apache-2.0 | [notice](licenses/rand_distr-0.4.3.txt) |
| rand_hc | 0.1.0 | MIT/Apache-2.0 | [notice](licenses/rand_hc-0.1.0.txt) |
| rand_isaac | 0.1.1 | MIT/Apache-2.0 | [notice](licenses/rand_isaac-0.1.1.txt) |
| rand_jitter | 0.1.4 | MIT OR Apache-2.0 | [notice](licenses/rand_jitter-0.1.4.txt) |
| rand_os | 0.1.3 | MIT/Apache-2.0 | [notice](licenses/rand_os-0.1.3.txt) |
| rand_pcg | 0.1.2 | MIT/Apache-2.0 | [notice](licenses/rand_pcg-0.1.2.txt) |
| rand_xorshift | 0.1.1 | MIT/Apache-2.0 | [notice](licenses/rand_xorshift-0.1.1.txt) |
| rawpointer | 0.2.1 | MIT/Apache-2.0 | [notice](licenses/rawpointer-0.2.1.txt) |
| rayon | 1.12.0 | MIT OR Apache-2.0 | [notice](licenses/rayon-1.12.0.txt) |
| rayon-core | 1.13.0 | MIT OR Apache-2.0 | [notice](licenses/rayon-core-1.13.0.txt) |
| rdrand | 0.4.0 | ISC | [notice](licenses/rdrand-0.4.0.txt) |
| redox_syscall | 0.1.57 | MIT | [notice](licenses/redox_syscall-0.1.57.txt) |
| regex | 1.13.1 | MIT OR Apache-2.0 | [notice](licenses/regex-1.13.1.txt) |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 | [notice](licenses/regex-automata-0.4.18.txt) |
| regex-syntax | 0.6.29 | MIT OR Apache-2.0 | [notice](licenses/regex-syntax-0.6.29.txt) |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 | [notice](licenses/regex-syntax-0.8.11.txt) |
| rust-lapper | 1.1.0 | MIT | [notice](licenses/rust-lapper-1.1.0.txt) |
| rustc_version | 0.1.7 | MIT OR Apache-2.0 | [notice](licenses/rustc_version-0.1.7.txt) |
| rustix | 1.1.5 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | [notice](licenses/rustix-1.1.5.txt) |
| rustversion | 1.0.23 | MIT OR Apache-2.0 | [notice](licenses/rustversion-1.0.23.txt) |
| rusty-fork | 0.2.2 | MIT/Apache-2.0 | [notice](licenses/rusty-fork-0.2.2.txt) |
| ryu | 1.0.23 | Apache-2.0 OR BSL-1.0 | [notice](licenses/ryu-1.0.23.txt) |
| safe_arch | 0.7.4 | Zlib OR Apache-2.0 OR MIT | [notice](licenses/safe_arch-0.7.4.txt) |
| semver | 0.1.20 | MIT/Apache-2.0 | [notice](licenses/semver-0.1.20.txt) |
| serde | 1.0.229 | MIT OR Apache-2.0 | [notice](licenses/serde-1.0.229.txt) |
| serde_core | 1.0.229 | MIT OR Apache-2.0 | [notice](licenses/serde_core-1.0.229.txt) |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 | [notice](licenses/serde_derive-1.0.229.txt) |
| serde_json | 1.0.151 | MIT OR Apache-2.0 | [notice](licenses/serde_json-1.0.151.txt) |
| sha2 | 0.10.9 | MIT OR Apache-2.0 | [notice](licenses/sha2-0.10.9.txt) |
| shlex | 2.0.1 | MIT OR Apache-2.0 | [notice](licenses/shlex-2.0.1.txt) |
| simba | 0.6.0 | Apache-2.0 | [notice](licenses/simba-0.6.0.txt) |
| simba | 0.9.1 | Apache-2.0 | [notice](licenses/simba-0.9.1.txt) |
| simd-adler32 | 0.3.10 | MIT | [notice](licenses/simd-adler32-0.3.10.txt) |
| simple-logging | 2.0.2 | BSD-3-Clause | [notice](licenses/simple-logging-2.0.2.txt) |
| skani | 0.3.2 | MIT OR Apache-2.0 | [notice](licenses/skani-0.3.2.txt) |
| smallvec | 1.16.2 | MIT OR Apache-2.0 | [notice](licenses/smallvec-1.16.2.txt) |
| statrs | 0.16.0 | MIT | [notice](licenses/statrs-0.16.0.txt) |
| statrs | 0.18.0 | MIT | [notice](licenses/statrs-0.18.0.txt) |
| strsim | 0.10.0 | MIT | [notice](licenses/strsim-0.10.0.txt) |
| strsim | 0.11.1 | MIT | [notice](licenses/strsim-0.11.1.txt) |
| strum | 0.25.0 | MIT | [notice](licenses/strum-0.25.0.txt) |
| strum_macros | 0.25.2 | MIT | [notice](licenses/strum_macros-0.25.2.txt) |
| strum_macros | 0.26.4 | MIT | [notice](licenses/strum_macros-0.26.4.txt) |
| syn | 1.0.109 | MIT OR Apache-2.0 | [notice](licenses/syn-1.0.109.txt) |
| syn | 2.0.119 | MIT OR Apache-2.0 | [notice](licenses/syn-2.0.119.txt) |
| syn | 3.0.6 | MIT OR Apache-2.0 | [notice](licenses/syn-3.0.6.txt) |
| tempfile | 3.27.0 | MIT OR Apache-2.0 | [notice](licenses/tempfile-3.27.0.txt) |
| termcolor | 1.3.0 | Unlicense OR MIT | [notice](licenses/termcolor-1.3.0.txt) |
| textwrap | 0.16.0 | MIT | [notice](licenses/textwrap-0.16.0.txt) |
| thiserror | 1.0.69 | MIT OR Apache-2.0 | [notice](licenses/thiserror-1.0.69.txt) |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | [notice](licenses/thiserror-2.0.21.txt) |
| thiserror-impl | 1.0.69 | MIT OR Apache-2.0 | [notice](licenses/thiserror-impl-1.0.69.txt) |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 | [notice](licenses/thiserror-impl-2.0.21.txt) |
| thread-id | 3.3.0 | MIT/Apache-2.0 | [notice](licenses/thread-id-3.3.0.txt) |
| tikv-jemalloc-sys | 0.5.4+5.3.0-patched | MIT/Apache-2.0 | [notice](licenses/tikv-jemalloc-sys-0.5.4+5.3.0-patched.txt) |
| tikv-jemallocator | 0.5.4 | MIT/Apache-2.0 | [notice](licenses/tikv-jemallocator-0.5.4.txt) |
| trilean | 1.1.0 | MIT/Apache-2.0 | [notice](licenses/trilean-1.1.0.txt) |
| triple_accel | 0.4.0 | MIT | [notice](licenses/triple_accel-0.4.0.txt) |
| typenum | 1.20.1 | MIT OR Apache-2.0 | [notice](licenses/typenum-1.20.1.txt) |
| unicode-ident | 1.0.26 | (MIT OR Apache-2.0) AND Unicode-3.0 | [notice](licenses/unicode-ident-1.0.26.txt) |
| utf8parse | 0.2.2 | Apache-2.0 OR MIT | [notice](licenses/utf8parse-0.2.2.txt) |
| vec_map | 0.8.2 | MIT/Apache-2.0 | [notice](licenses/vec_map-0.8.2.txt) |
| version_check | 0.9.5 | MIT/Apache-2.0 | [notice](licenses/version_check-0.9.5.txt) |
| wait-timeout | 0.2.1 | MIT/Apache-2.0 | [notice](licenses/wait-timeout-0.2.1.txt) |
| wasi | 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | [notice](licenses/wasi-0.11.1+wasi-snapshot-preview1.txt) |
| wide | 0.7.33 | Zlib OR Apache-2.0 OR MIT | [notice](licenses/wide-0.7.33.txt) |
| winapi | 0.3.9 | MIT/Apache-2.0 | [notice](licenses/winapi-0.3.9.txt) |
| winapi-i686-pc-windows-gnu | 0.4.0 | MIT/Apache-2.0 | [notice](licenses/winapi-i686-pc-windows-gnu-0.4.0.txt) |
| winapi-util | 0.1.6 | Unlicense/MIT | [notice](licenses/winapi-util-0.1.6.txt) |
| winapi-x86_64-pc-windows-gnu | 0.4.0 | MIT/Apache-2.0 | [notice](licenses/winapi-x86_64-pc-windows-gnu-0.4.0.txt) |
| windows-link | 0.2.1 | MIT OR Apache-2.0 | [notice](licenses/windows-link-0.2.1.txt) |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 | [notice](licenses/windows-sys-0.61.2.txt) |
| xz2 | 0.1.7 | MIT/Apache-2.0 | [notice](licenses/xz2-0.1.7.txt) |
| zerocopy | 0.8.62 | BSD-2-Clause OR Apache-2.0 OR MIT | [notice](licenses/zerocopy-0.8.62.txt) |
| zerocopy-derive | 0.8.62 | BSD-2-Clause OR Apache-2.0 OR MIT | [notice](licenses/zerocopy-derive-0.8.62.txt) |
| zlib-rs | 0.6.8 | Zlib | [notice](licenses/zlib-rs-0.6.8.txt) |
| zmij | 1.0.23 | MIT | [notice](licenses/zmij-1.0.23.txt) |
