# ANIm development fixtures

These files are byte-for-byte copies from the frozen issue #1 synthetic feasibility fixture set, generated with seed 20261009. They contain no study genomes. The original manifest SHA-256 is `3879a2fa3f8d9c315ff66712b0477e5e4c286455c5c7775ce28c91117a255668`. Full generation commands and receipts remain in the research run.

| File | SHA-256 |
| --- | --- |
| `identical.q.fa` | `253cdd6edc1e68e54f8035ab952b92dd1a4b92587b851e5782207f7ff4f869a3` |
| `identical.r.fa` | `17d999592d8ca0f5ec6180e94d9b34338e20bc2e7d81275ce788b6c8a2553b60` |
| `af_800.q.fa` | `d021cd9e8cdcf26a4cdbb0c3f56f07894e2e2d219e4f63c4043001d04c40a485` |
| `af_800.r.fa` | `17d999592d8ca0f5ec6180e94d9b34338e20bc2e7d81275ce788b6c8a2553b60` |
| `no_alignment.q.fa` | `4341e3f436ef98f567cafc0f81a733f5a5b428cfc9970e57ad89be5c98814001` |
| `no_alignment.r.fa` | `17d999592d8ca0f5ec6180e94d9b34338e20bc2e7d81275ce788b6c8a2553b60` |
| `ambiguous.q.fa` | `1af62994881d28cff78cf408c8acd081161a5127971855d252a51446500aa0b1` |
| `ambiguous.r.fa` | `17d999592d8ca0f5ec6180e94d9b34338e20bc2e7d81275ce788b6c8a2553b60` |

Parser tests also derive inclusive overlap, reverse and weighted cases from the frozen P1 contract fixtures. The singleton and invalid coordinate cases exercise the same integer contract at its boundaries.

`cropped.q.fa` takes the first 96000 bases from `identical.q.fa` and renames the record to Q. Its SHA-256 is `01ddd8ba4830379eba1e8169367122fac75f014f667cfc07179aa33547bc59ee`. This derivation exposes asymmetric whole-input lengths without adding new sequence evidence.

The P1 filtered-delta fixtures below are byte-for-byte copies from the frozen issue #20 contract. They use native IDs R and Q; overlap declares lengths 20/20 and reverse/weighted declare 40/40. Weighted is an abstract metric fixture whose unequal spans have a zero offset terminator; the parser preserves its header metrics.

| File | SHA-256 |
| --- | --- |
| `overlap.filter` | `62ad9c6a9421986f2f6221874941d21469001e7831ca1ee67b31648bad0a7b7e` |
| `reverse.filter` | `fcda4be8817e924ece9b3bd85221d884483241e0a8f6ee4c8d83f887ec63c8e3` |
| `weighted.filter` | `1f8776b68a2c06070e334847ddc28062772298e3cf4d75840d85d98e54080442` |

The CRLF regression rewraps the frozen identical query sequence to ten bases per line and surrounds the reference sequence lines with spaces and tabs. It retains all 120000 bases, descriptions and original-byte hashes, and expects full identical coverage in both orientations.
