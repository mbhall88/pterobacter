# Maintained MUMmer4 source

This subset comes from MUMmer4 v4.0.1, commit `eb734606f2d516f42a0e0dce7a116bfb88ec1ebf`. `upstream-files.json` records the original SHA-256 of each selected upstream file. The eight compiled translation units are listed in `build.rs`; the remaining files are headers and upstream notices.

The only upstream code changes are the five lines in `investigations/anim/mummer4-maintenance.patch`, approved in PR #18:

- Preserve reference Ns rather than replacing them with pseudorandom nucleotides.
- Write delta output to the caller's stream at three sites.
- Correct the public `align_file` template parameter.

`native/anim.cc` supplies the Terobacter bridge. Alignment and filtering use the pinned library directly; no MUMmer executable is required at runtime. A worker process of the current Terobacter executable contains each native orientation.

The upstream package includes the Artistic License 2.0 in `LICENSE.md`. Jellyfish and mt_skip_list headers carry GPLv3-or-later notices; `GPL-3.0.txt` supplies that license. Compact suffix sorting and thread_pipe files retain their own permissive notices. `COPYRIGHT` retains the upstream package notice. Terobacter's root MIT license describes its own code and does not replace these component licenses.

The combined Pterobacter executable is distributed under GPL-3.0-or-later conditions. Original component grants remain intact; the Artistic-2.0 compatibility route is section 4(c)(ii). Every distribution must retain the applicable notices and make complete corresponding source available, including this maintained subset, its upstream-file manifest and the maintenance patch. See [licensing and source availability](../../docs/licensing.md). This distribution policy does not itself publish a release.
