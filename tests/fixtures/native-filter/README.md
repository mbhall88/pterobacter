# Synthetic filter tie regression

The delta file contains three invented, perfect 100-base alignments. Each maps the same query interval to a different reference interval. There are no biological sequences or study-derived records. The expected filter retains reference interval 101–200. With upstream libc tie handling, glibc seed 1 selects that interval, whereas Darwin seed 1 selects 501–600.

`check.cc` exercises the real `DeltaGraph_t::build`, `flag1to1` and `outputDelta` path. It checks the first ten known glibc seed-1 outputs, a little-endian FNV-1a fingerprint over 100,000 outputs, a 100,000-draw differential against libc only when compiling on glibc, repeated orientation resets and independence from libc state. macOS uses the same fixed expected outputs, never its libc as the expected sequence.

The test runs from `just test` on both CI platforms. The maintained generator is Pterobacter-authored code; no glibc source was copied.
