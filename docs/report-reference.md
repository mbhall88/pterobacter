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

### Input identity and sample names

`input.file` is the assembly path that was read. It is also retained as the skani query label for provenance. Exact orientation-score ties use sketch content, so filenames do not select the measurement orientation. `input.query_sha256` and the per-record hashes under `input.records` identify the bytes and sequences, and `input.ordinal` gives the position in the run.

`input.sample_name` holds the name given with `--sample-name` or in a sample sheet, and is null otherwise. It is reporting metadata only and takes no part in the analysis. `input.source` records where the path came from: `{"kind": "positional"}` for a command-line argument, or the kind `input_list` or `sample_sheet` with the list's `file`, the 1-based `line` and the `entry` exactly as written there. A relative entry is resolved against the list's directory to give `input.file`.

`calls.tsv` and `evidence.tsv` carry the name in their second column, `sample_name`, and leave it empty for unnamed inputs. `report.md` heads each section with the sample name and shows the input file below it. Markdown and HTML characters in a name are escaped there so it displays exactly as written; the JSON and TSV files keep the name unchanged. Join the two TSV files on `assembly`, the input's position, which is always filled.

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

The [ECC-219 example](examples/public/unresolved.md) is an actual public assembly that stays unresolved after ANIm. Its [saved report](examples/public/batch/report.json) and [reproduction commands](examples/README.md#run-them-yourself) retain the whole input and database identity.

| Field | Observed value |
|---|---|
| `status` | `unresolved` |
| `reasons` | `ANIm_no_strict_witness` |
| Nearest species reference | *E. nematophilus*, GCA_026344075.1 |
| skani ANI | 94.83% |
| Reciprocal ANIm | 95.02965% / 95.03431% |
| ANIm pairs completed / required | 1 / 1 |
| MLST | ST2274 |

**Read the reason first.** The alignment ran successfully, but neither orientation exceeded the required 96% ANI. This is not a missing comparison or an execution failure. Adequate aligned fraction and an exact ST do not substitute for the species criterion.

**Keep the other candidates visible.** The strongest-match and whole-panel tables include the nearby *E. bugandensis* references as well as *E. nematophilus*. Do not turn the nearest-reference name into an assignment. An empty challenge list is not proof that every alternative has been ruled out: read the competitor-assessment field and the measured pair counts.

**Choose a follow-up question.** A useful next question is whether broader sampling places ECC-219 within an existing group or supports a separate lineage. Select types and diverse representatives of both nearby groups, adding relevant neighbouring groups for context. The [boundary guide](boundary-review.md#tree-follow-up) explains how to record that selection and compare tree evidence. No follow-up tree or new species conclusion is claimed for this documentation example.

**Preserve the automatic result.** Record any later expert interpretation separately, with its inputs, evidence and rationale. The saved output stays species-unresolved, with its independently reported ST2274.

## Limits

- Pterobacter takes one isolate assembly per input. Metagenomes and mixed samples are out of scope, and a clean report does not prove the assembly is pure.
- Quality control is your responsibility. Pterobacter runs no CheckM2 or other quality check on a query.
- Reads are not supported. Input kind and provenance stay explicit in the reports so that read support can be added later with its own validation.
- MLST covers PubMLST submissions through 31 December 2024. It never sets a taxon.
- A genomovar call is a conditional match to one frozen group, not a population boundary. An unrepresented group can match a neighbouring reference group. Overlapping compatible groups are withheld; singleton matches retain a sparse-reference warning. See [genomovar.md](genomovar.md).
