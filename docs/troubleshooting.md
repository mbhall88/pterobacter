# Troubleshoot a run

Check the terminal error and the execution status for every input before interpreting a missing call. A completed unresolved result is a biological withholding; a failed run is not.

| Symptom | Check |
|---|---|
| GNU compiler error during installation | Use GNU C++ with C++17 support. On macOS set `CXX` to Homebrew's `g++`; the native build refuses Apple clang. |
| Reference digest or file mismatch | Use the intact directory and trusted digest from the same handoff. Do not bypass a failure by hashing an untrusted replacement and trusting that value. |
| Existing output path or path collision | Choose new output names outside reference-package directories. Report, alignment and cache paths must be distinct and not nested. |
| Assembly rejected | Supply a nonempty nucleotide FASTA with identifiers and nonempty records. FASTQ and reads are unsupported. |
| No assigned name after a completed run | Read the reasons and comparisons. Do not replace an empty assignment with the nearest reference. See [uncertain results](boundary-review.md). |
| Candidate ANIm run needs more data | Supply the matching authenticated reference FASTA component and a new evidence directory. A failed required alignment does not fall back to a skani-only call. |
| MLST failed | Check the scheme digest, allele states and error. Completed classification can remain available even when typing fails. |

Exit 0 means all inputs finished, including unresolved results. A nonzero exit means a failure occurred; a batch may still contain completed inputs. Errors before analysis can leave no report, while later failures retain evidence where possible. The [exit-code table](assembly.md#reports-and-failures) gives the precise meanings and precedence.

When reporting a problem, retain the command, software/reference identities, error and relevant report fields. Share only data you are authorised to disclose.
