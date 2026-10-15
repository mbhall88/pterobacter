# P4a-r3 command binding

> Historical contract: command spellings and defaults below record the original evaluation interface. The current development executable is `pterobacter-dev`, built with `cargo build --locked --release --features dev --bin pterobacter-dev`. Use the [development guide](../development.md) and [current CLI help](../cli-help.md) for present commands and defaults; scientific profile identifiers retain their original names.

This binding accompanies `P1-ANIM-MUMMER4-r1`. It leaves the frozen P4a command plan unchanged and maps its R3 placeholder to the development binary. `mbhall88/Enterobacter#25` must bind the actual source/build/executable digests and parser fixtures before launching validation.

```sh
cargo build --locked --features dev --bin terobacter-dev
terobacter-dev anim-pair \
  --query QUERY.fa --reference REFERENCE.fa \
  --expect-query-sha256 QUERY_SHA256 \
  --expect-reference-sha256 REFERENCE_SHA256 \
  --query-id QUERY_ID --reference-id REFERENCE_ID \
  --reference-component COMPONENT \
  --reference-concept-id CONCEPT_ID \
  --output NEW_DIRECTORY
```

For P4's `anim_pair_argv`, replace the tool placeholder with the absolute path to the pinned `terobacter-dev` executable. Existing query/reference/hash/output placeholders keep their meanings. Package identity options bind the selected component and concepts; repeat `--reference-concept-id` for multiple concepts. Exit 0 means a complete reciprocal measurement, including successful no alignment. Exit 1 means incomplete evidence or an operation error. The P4 wrapper retains its own exit codes and attempt/archive rules.

EN-4 calls the same production `parse_delta` function through the development parser operation. Whole-input length maps are required because filtered delta headers cannot describe unaligned contigs:

```sh
terobacter-dev anim-parse --filtered-delta FILE \
  --reference-lengths REFERENCE_LENGTHS.json --query-lengths QUERY_LENGTHS.json
```

The length files are JSON objects from contig ID to positive integer length. For the frozen P1 fixtures they are `{"R":20}` and `{"Q":20}` for overlap, and `{"R":40}` and `{"Q":40}` for reverse and weighted; compare the emitted integer fields with the frozen contract model. Parser outputs describe native axes and carry no reciprocal pair completion claim.

The public `tero` and `terobacter` commands retain their existing syntax. Build, parser, cancellation, Linux parity, macOS parity and licensing evidence are separate from biological acceptance. This mapping supplies R3 and EN-4; it does not launch P4 or implement its wave scheduler.
