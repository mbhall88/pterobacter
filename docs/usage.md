# Using Terobacter

Terobacter classifies an *Enterobacter* isolate assembly against a versioned reference package. It reports a species and subspecies only when the approved rules support one, and it always shows the evidence behind the answer. This guide covers installing the tool, running it, reading its reports and following up an isolate that it leaves unresolved.

The tool is in development. No release version is chosen, and the candidate v2 policy reports `not_yet_validated` until its evaluation is accepted.

## Install

There is no packaged release yet. Build from source:

```sh
cargo install --path . --locked
```

You need a recent stable Rust toolchain and GNU C++ with C++17 support, because the ANIm step embeds native code. On macOS, install Homebrew GCC and set `CXX` to its `g++`. The install puts two commands on your path, `tero` and `terobacter`. They are the same program; use `terobacter` if another tool already uses `tero`.

A run also needs a reference package: a directory of skani databases and tables, listed in a manifest. Obtain it from a trusted source together with the SHA-256 of its `portable.json`. The command refuses a package whose digest differs. Once the package is on disk, a run needs no network and no other program.

The candidate v2 policy needs a second directory when it has to run ANIm: the reference FASTA component, with its own trusted `anim.json` digest. See [whole-assembly classification](assembly.md#separate-reference-fasta-component).

## Run

```sh
tero --reference-package REFERENCE_DIR --expected-sha256 PORTABLE_JSON_SHA256 \
  --report-dir results isolate.fa
```

Pass the whole assembly as you have it. Plasmids and short records stay in; you do not need to pick out the chromosome. Several assemblies can follow one command, and each gets its own report and its own status. A failure in one does not erase another.

| Option | Use |
|---|---|
| `--reference-package DIR`, `--expected-sha256 SHA` | The reference package and the digest you trust for it. Both are required. |
| `--policy NAME` | `conservative-assembly-v1` (default) or `conservative-assembly-anim-v2`. |
| `--threads N` | Worker threads for each assembly. |
| `--jobs N` | Analyse up to N assemblies at once, each with its own threads. |
| `--output FILE` | Write the main JSON to a new file instead of standard output. |
| `--anim-cache DIR` | Reuse verified v2 alignments across runs. |
| `--report-dir DIR` | Write the four report files into `DIR`, which must not exist yet and must not sit inside an input package. |
| `--mlst-package DIR`, `--mlst-expected-sha256 SHA` | Add optional historical MLST. It never changes the classification. |
| `--anim-reference-package DIR`, `--expected-anim-sha256 SHA`, `--anim-output DIR` | Provide the reference FASTAs to the v2 policy and keep its alignment evidence in a new directory. |

The main JSON report goes to standard output unless `--output FILE` is supplied. `--report-dir` adds the four report files without changing that destination. ANIm evidence and cache files use their own options.

The exit status is 0 when every input finished, including inputs that ended unresolved. Any other status means some part of a run failed; [the exit-code table](assembly.md#reports-and-failures) lists them. A non-zero status still leaves reports behind where it can: a failed input gets a row and a section that say so.

## Outputs

`--report-dir` writes four files. If the directory exists, or lies inside the reference, MLST or ANIm reference package, the command exits 2 before doing any analysis and leaves it alone. It never replaces a file. If a bundle file cannot be written, the main JSON still goes to its chosen destination and the command exits 1. If writing `--output FILE` fails, the main JSON falls back to standard output. Output destinations must be distinct and must not sit inside one another; see [output-path checks](assembly.md#reports-and-failures).

| File | What it holds |
|---|---|
| `report.json` | The authoritative report. One object for one input, an array for several. |
| `calls.tsv` | One row per assembly: the final call, reasons, policy, minima, method, counts and versions. |
| `evidence.tsv` | One row per evidence item: the strongest matches, each parent in the reference panel, and each challenge. |
| `report.md` | A readable summary of the same facts, for manual review. |

The TSV and Markdown files are produced from the JSON. They state the same final calls, reasons, policy, minima and method, and they cannot show a label the JSON does not assign. If you find a difference, the JSON is right, and the difference is a bug worth reporting.

### The final call

`call_status` is `assigned`, `unresolved` or `not_evaluated`.

- `assigned`: the approved rules support a species. `species_id` and `species_name` are filled. Subspecies and genomovar are separate calls and can stay empty while the species is assigned.
- `unresolved`: the run finished and the evidence does not support a call. The species, subspecies and genomovar columns are empty. `reasons` says why, using the terms in [`reason-vocabulary.tsv`](contracts/reason-vocabulary.tsv).
- `not_evaluated`: the run did not reach a decision. Look at `execution_status`, `execution_stage` and `execution_error`. Do not read this as a biological negative.

A label never appears as a call unless the decision assigned it. The nearest parent, the original v1 outcome and a rescue witness do appear, but each under a name that says it is not the call: "Nearest parent (not an assignment)", `original_v1_*` columns, and so on.

### Evidence you can check

The report shows the five highest skani references and every exact tie at fifth place, so a tie is never cut at an arbitrary row. Rows in one `tie_group` have the same ANI. They are sorted by reference ID only so the output is stable; the order does not pick a winner. Each reference is a competitor, not a vote.

The top table does not cover the whole panel. The panel summary lists every parent in the reference panel, whether skani returned it or not, with its best ANI and any challenging references. A challenge outside the top table appears there and in the challenges list, marked as outside the strongest matches.

The v1 policy has no foreign-parent challenge test, so its challenge list is always empty; compare each parent's best ANI in the panel summary instead.

For the v2 policy, an empty challenge list is a result only when the check was carried out. `challenge_assessment` says how far it went:

| Value | Meaning |
|---|---|
| `completed` | Every required competitor pair was measured. An empty list means the retained measurements contain no challenge; this alone does not validate the call. |
| `partial` | Some competitor pairs are missing. Known challenges are retained; an unlisted reference has not been cleared. |
| `not_assessed` | The competitor panel was not assessed. An empty challenge list is not negative evidence. |
| `not_applicable` | The policy did not require a competitor check; for example, the query was outside the rescue band or lacked a qualifying skani coverage witness. V1 has no such check. |

Assessment coverage is separate from `execution_status`. A failed execution or integrity check can retain measurements while withholding the call. Check both fields.

ANI methods are not mixed in one ranking. skani orders the table; ANIm values, where the v2 policy measured them, sit beside it as separate columns.

### Counts and versions

For the v2 policy, `anim_required`, `anim_attempted`, `anim_completed`, `anim_no_alignment`, `anim_failed` and `anim_not_run` say how many reciprocal ANIm pairs the decision needed and what happened to each. A pair that aligned nothing is a result and counts under `anim_no_alignment`. A pair that did not run, or failed, leaves the competitor panel incomplete, and an incomplete panel blocks a rescue. `competitor_evaluation` says which. The skani columns say how many references were in the panel, how many skani returned and how many it did not.

Each report carries its versions separately, in the `versions` object and in the matching columns. They answer different questions, so do not merge them:

| Field | Identifies |
|---|---|
| `implementation` | The program build. |
| `report_schema` | The shape of the report. |
| `policy`, `policy_contract` | The calling rules and the contract revision behind them. |
| `validation` | Whether the policy has passed its evaluation. Null for v1, `not_yet_validated` or `custom_unvalidated` for v2. |
| `skani_method`, `anim_method` | The measurement method profiles. |
| `reference_data` | The reference package version and manifest SHA-256. |

## A worked manual-review example

This example uses invented numbers. It follows an isolate that the v2 policy leaves unresolved, and shows what to do next without inventing a label.

You run:

```sh
tero --reference-package REFERENCE_DIR --expected-sha256 PORTABLE_JSON_SHA256 \
  --policy conservative-assembly-anim-v2 \
  --anim-reference-package REFERENCE_FASTAS --expected-anim-sha256 ANIM_SHA256 \
  --anim-output ani-evidence --report-dir review isolate_17.fa
```

`calls.tsv` has one row. The columns that matter:

| Column | Value |
|---|---|
| `call_status` | `unresolved` |
| `reasons` | `foreign_parent_challenge` |
| `original_v1_status` | `unresolved` |
| `original_v1_reasons` | `nearest_below_species_ANI` |
| `trigger` | `true` |
| `rescue_status` | `withheld` |
| `anim_completed` / `anim_required` | `5` / `5` |

The isolate sat just under the species threshold, so v1 left it unresolved and v2 tried to rescue it with ANIm. v2 withheld the rescue because another parent challenged it.

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

- Terobacter takes one isolate assembly per input. Metagenomes and mixed samples are out of scope, and a clean report does not prove the assembly is pure.
- Quality control is your responsibility. Terobacter runs no CheckM2 or other quality check on a query.
- Reads are not supported. Input kind and provenance stay explicit in the reports so that read support can be added later with its own validation.
- Optional historical MLST covers PubMLST submissions through 31 December 2024. It never sets a taxon.
- A genomovar call is a conditional match to one frozen group, not a population boundary. See [genomovar.md](genomovar.md).
