# User guide

Start with a whole isolate assembly in FASTA format. Terobacter keeps all supplied records, including plasmids and short contigs, and reports the most specific supported call with its reasons.

| Task | Guide |
|---|---|
| Install from the private source | [Installation](installation.md) |
| Obtain and verify the reference data | [Reference setup](reference-setup.md) |
| Analyse one assembly or a batch | [Running Terobacter](running.md) |
| Find the answer and warnings | [Reading results](results.md) |
| Investigate an uncertain result | [Boundary review and worked example](boundary-review.md) |
| Check which taxa are represented | [Reference coverage](supported-taxa.md) |
| Add historical sequence typing | [Historical MLST](historical-mlst.md) |
| Interpret a genomovar match | [Genomovar matches](genomovar.md) |
| Resolve a failed run | [Troubleshooting](troubleshooting.md) |

Use one isolate assembly per input. Reads, metagenomes and mixed samples are outside the supported input. Query quality control is your responsibility; the program does not run CheckM2 and a completed report does not certify purity.

The [full command-line help](cli-help.md) lists all options exactly as `tero --help` prints them.

The [report reference](report-reference.md) describes every output field and includes a separate illustrative manual-review recipe. The [command reference](assembly.md) covers advanced ANIm settings, cache verification and exit codes. These use the current interface; proposed interface changes have not been implemented.
