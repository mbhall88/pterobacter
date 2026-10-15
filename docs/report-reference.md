# Report field reference

Use this reference to check exact output fields, evidence completeness and version identities, or follow the illustrative manual-review recipe. For a first run, use [running assemblies](running.md) and [reading results](results.md).

## Outputs

`--report-dir` writes four files. If the directory exists, or lies inside the reference, MLST or ANIm reference package, the command exits 2 before doing any analysis and leaves it alone. It never replaces a file. If a bundle file cannot be written, the main JSON still goes to its chosen destination and the command exits 1. If writing `--output FILE` fails, the main JSON falls back to standard output. Output destinations must be distinct and must not sit inside one another; see [output-path checks](assembly.md#reports-and-failures).

| File | What it holds |
|---|---|
| `report.json` | The authoritative report. One object for one input, an array for several. |
| `calls.tsv` | One row per assembly: the final call, reasons, settings, minima, method, counts and versions. |
| `evidence.tsv` | One row per evidence item: the strongest matches, each parent in the reference panel, and each challenge. |
| `report.md` | A readable summary of the same facts, for manual review. |

The TSV and Markdown files are produced from the JSON. They state the same final calls, reasons, settings, minima and method, and they cannot show a label the JSON does not assign. If you find a difference, the JSON is right, and the difference is a bug worth reporting.

### Data selection

Native reports add `data_provenance`: the actual reference trust source, selected
catalogue/release/citation when managed, explicit or catalogue-derived component
pins, typing request status and the retained ANIm evidence directory. Each
rescue-mode run uses a unique directory unless an explicit new output path is
supplied. This path and catalogue metadata are transport provenance, separate
from scientific measurements and package identities. Markdown includes the same
selection metadata. See [database management](databases.md).

### The final call

`call_status` is `assigned`, `unresolved` or `not_evaluated`.

- `assigned`: the approved rules support a species. `species_id` and `species_name` are filled. Subspecies and genomovar are separate calls and can stay empty while the species is assigned.
- `unresolved`: the run finished and the evidence does not support a call. The species, subspecies and genomovar columns are empty. `reasons` says why, using the terms in [`reason-vocabulary.tsv`](contracts/reason-vocabulary.tsv).
- `not_evaluated`: the run did not reach a decision. Look at `execution_status`, `execution_stage` and `execution_error`. Do not read this as a biological negative.

A label never appears as a call unless the decision assigned it. The nearest parent, the initial skani result and a rescue witness do appear, but each under a name that says it is not the call: "Nearest parent (not an assignment)" and "Initial skani result".

### Evidence you can check

The report shows the five highest skani references and every exact tie at fifth place, so a tie is never cut at an arbitrary row. Rows in one `tie_group` have the same ANI. They are sorted by reference ID only so the output is stable; the order does not pick a winner. Each reference is a competitor, not a vote.

The top table does not cover the whole panel. The panel summary lists every parent in the reference panel, whether skani returned it or not, with its best ANI and any challenging references. A challenge outside the top table appears there and in the challenges list, marked as outside the strongest matches.

With `--no-anim`, there is no alignment-based foreign-parent challenge test, so its challenge list is always empty; compare each parent's best ANI in the panel summary instead.

With ANIm enabled, an empty challenge list is a result only when the check was carried out. `challenge_assessment` says how far it went:

| Value | Meaning |
|---|---|
| `completed` | Every required competitor pair was measured. An empty list means the retained measurements contain no challenge; this alone does not validate the call. |
| `partial` | Some competitor pairs are missing. Known challenges are retained; an unlisted reference has not been cleared. |
| `not_assessed` | The competitor panel was not assessed. An empty challenge list is not negative evidence. |
| `not_applicable` | The analysis did not require a competitor check; for example, the query was outside the rescue band or lacked a qualifying skani coverage witness. The check is also skipped with `--no-anim`. |

Assessment coverage is separate from `execution_status`. A failed execution or integrity check can retain measurements while withholding the call. Check both fields.

ANI methods are not mixed in one ranking. skani orders the table; ANIm values, where measured, sit beside it as separate columns.

### Counts and versions

With ANIm enabled, `anim_required`, `anim_attempted`, `anim_completed`, `anim_no_alignment`, `anim_failed` and `anim_not_run` say how many reciprocal ANIm pairs the decision needed and what happened to each. A pair that aligned nothing is a result and counts under `anim_no_alignment`. A pair that did not run, or failed, leaves the competitor panel incomplete, and an incomplete panel blocks a rescue. `competitor_evaluation` says which. The skani columns say how many references were in the panel, how many skani returned and how many it did not.

Each report carries its versions separately, in the `versions` object and in the matching columns. They answer different questions, so do not merge them:

| Field | Identifies |
|---|---|
| `implementation` | The program build. |
| `report_schema` | The shape of the report. |
| `validation` | Evaluation status: unset with ANIm disabled; otherwise `not_yet_validated` or `custom_unvalidated`. |
| `skani_method`, `anim_method` | The measurement method profiles. |
| `reference_data` | The reference package version and manifest SHA-256. |

## A worked manual-review example

This example uses invented numbers. It follows an isolate that remains unresolved after ANIm, and shows what to do next without inventing a label.

You run:

```sh
ptero --reference-package REFERENCE_DIR --expected-sha256 PORTABLE_JSON_SHA256 \
  --anim-reference-package REFERENCE_FASTAS --expected-anim-sha256 ANIM_SHA256 \
  --anim-output ani-evidence --report-dir review isolate_17.fa
```

`calls.tsv` has one row. The columns that matter:

| Column | Value |
|---|---|
| `call_status` | `unresolved` |
| `reasons` | `foreign_parent_challenge` |
| `trigger` | `true` |
| `rescue_status` | `withheld` |
| `anim_completed` / `anim_required` | `5` / `5` |

The initial skani estimate was just below the species threshold, so the tool ran ANIm. The result stayed unresolved because another parent challenged the call.

**Step 1: read the reasons.** `foreign_parent_challenge` means a reference from a different parent also reached the conflict threshold. The pair counts are complete, so this is a measured disagreement, not a missing measurement.

**Step 2: find the competitor.** In `report.md`, the Strongest matches table puts species A at 95.29% and species B at 95.15%. The Challenges section names `primary_whole:ACCESSION_B1` of species B, marked as outside the strongest matches: skani ranked it low, but both ANIm orientations reach the conflict threshold. The Whole-panel summary lists species B with that reference under "Challenging references", so the challenge is visible even though the top table alone would hide it.

**Step 3: collect the genomes.** Reference IDs are GenBank assembly accessions after the `primary_whole:` prefix. Put the accessions of the isolate's strongest references and of the challenging reference in a file, then download them with NCBI Datasets:

```sh
datasets download genome accession --inputfile accessions.txt --include genome
```

Unpack them into one directory with `isolate_17.fa`. Add a type anchor for each parent; the `reference_role` column in `evidence.tsv` marks them.

**Step 4: repeat the ANI.** A reproduction is only as exact as the settings you pin. Read them from the report first. `measurement.engine_version` and `measurement.engine_profile` (also `versions.skani_method`) name the skani build, `0.3.2` here, and `measurement.reporting_floor_percent` gives the aligned-fraction floor. The package search also screens at 80% and takes its sketch compression from the reference package's database, not from a skani default. Use the same inputs: the isolate file must hash to `input.query_sha256` in the report, and a reference downloaded from NCBI may differ from the copy in the package if the assembly was updated, so use the package's reference FASTA component when you have it. The isolate is always the query and the reference genome is the reference. skani's ANI and aligned fractions are not symmetric, so swapping them changes the numbers.

With the same skani version, `skani dist` on the isolate and one reference checks that pair. It is not an exact replay of the package search, so a default invocation does not guarantee the same value:

```sh
skani --version    # must report 0.3.2
skani dist -q isolate_17.fa -r refs/ACCESSION_A1.fa -o follow_up_ani.tsv
```

Compare `ANI` with `skani_ani`, and `Align_fraction_query` with `skani_af_query`, in the `evidence.tsv` row for that reference. Small differences are expected unless you also match the database's compression settings. A large difference points at the wrong genome, the wrong orientation or a different skani build.

For an independent view, run a different ANI method on the same pairs, such as fastANI or a whole-genome alignment, and compare its values and aligned fractions.

If the follow-up agrees that species A and species B are both near 95%, the isolate sits between two parents and the tool was right to withhold. If the follow-up puts one parent clearly ahead, check the isolate for contamination or a second genome: compare the record lengths and sequence hashes in `report.json` under `input.records` with what you expect.

**Step 5: place it on a tree.** Build a quick tree of the isolate, the references from both parents and the type anchors:

```sh
mashtree --numcpus 4 isolate_17.fa refs/*.fa > follow_up.dnd
```

Open `follow_up.dnd` in a tree viewer. Three patterns are common. The isolate nests inside one parent's references: that parent is the likely answer and the challenge probably reflects a close relative. The isolate sits on a long branch between the two: it may belong to a group the package does not represent. It sits with neither: the reference panel may not cover it. A whole-genome distance tree is quick, not authoritative. For anything you plan to publish, build a core-genome tree with a method suited to the question.

**Step 6: record the outcome.** The result for this isolate stays unresolved. Write down what you found: "closest to species A at 95.29%, challenged by species B at 95.15%, tree placement …". Do not copy the nearest parent into a species field. If the evidence points to a lineage the package lacks, that is a question for the reference package's maintainers, not something the tool decides.

## Limits

- Pterobacter takes one isolate assembly per input. Metagenomes and mixed samples are out of scope, and a clean report does not prove the assembly is pure.
- Quality control is your responsibility. Pterobacter runs no CheckM2 or other quality check on a query.
- Reads are not supported. Input kind and provenance stay explicit in the reports so that read support can be added later with its own validation.
- Optional historical MLST covers PubMLST submissions through 31 December 2024. It never sets a taxon.
- A genomovar call is a conditional match to one frozen group, not a population boundary. An unrepresented group can match a neighbouring reference group. Overlapping compatible groups are withheld; singleton matches retain a sparse-reference warning. See [genomovar.md](genomovar.md).
