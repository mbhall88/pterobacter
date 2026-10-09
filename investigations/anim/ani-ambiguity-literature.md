# Ambiguous nucleotide handling in ANI: focused source check

Checked on **2026-10-09**. This is a targeted narrative check for a Terobacter method discussion, not a systematic review or a claim that the entire ANI literature has been exhausted. No ANI experiments or method changes were performed for this note.

**Subsequent decision, 2026-10-09:** Michael chose to match MUMmer3 output for now,
accepting the known long-N risk. Ambiguity-column exclusion remains an option
discussed here, rather than the adopted metric. See the
[compatibility decision](mummer4-maintenance.md#accepted-compatibility-target).

## Finding and relevance

There is explicit **published ANI precedent for filtering N-rich fragments**: the 2025 PyOrthoANI paper discards fragments containing more than 80% Ns. There is also explicit **first-party alignment documentation for excluding ambiguous columns from both the identity numerator and denominator**: LASTZ. These support different operations. Fragment rejection changes which regions are compared; column exclusion changes the identity statistic within an accepted alignment. [PyOrthoANI methods](https://pmc.ncbi.nlm.nih.gov/articles/PMC12246781/#sec2), [LASTZ back-end filtering](https://lastz.github.io/lastz/#back-end-filtering).

The inspected foundational ANI methods do not spell out a common N-to-N versus N-to-known, per-column identity policy. This bounded observation does **not** establish that such a policy has never been published. BLAST/MUMmer error semantics, downstream parsing, and software version must be checked separately before describing the policy of a particular implementation. [Goris et al. methods](https://www.microbiologyresearch.org/content/journal/ijsem/10.1099/ijs.0.64483-0?crawler=true&mimetype=application/pdf), [Richter and Rosselló-Móra methods](https://pmc.ncbi.nlm.nih.gov/articles/PMC2776425/), [Lee et al. methods](https://www.microbiologyresearch.org/content/journal/ijsem/10.1099/ijsem.0.000760?crawler=true&mimetype=application/pdf).

MUMmer4's reference-N randomization has an explicit upstream rationale: preventing long N stretches from aligning. It was introduced in September 2020, after the 2018 paper, and qualifies the earlier prototype that removed substitution. See the source/history section below. [Introducing commit](https://github.com/mummer4/mummer/commit/017e5dfda38a174df649fc7fcfbcc3113cefcc75).

For Terobacter, the defensible claim is therefore **a motivated, explicitly defined treatment of missing base information**, with partial ANI precedent and direct adjacent aligner precedent. It should not be presented as an already demonstrated universal ANI standard. This is an interpretation of the sources, not a published Terobacter validation result.

## Sources inspected

| Primary source; verified year and DOI | Material inspected | Evidence relevant to ambiguity |
|---|---|---|
| Goris et al. (2007), *DNA-DNA hybridization values and their relationship to whole-genome sequence similarities*. [10.1099/ijs.0.64483-0](https://doi.org/10.1099/ijs.0.64483-0) | Publisher full PDF; Methods, pp. 82–84, especially Sequence-based comparisons; Discussion; text search for ambiguity/unknown | BLAST-based fragment ANI and separate conserved-DNA fraction; no explicit N-column rule found in those sections. |
| Richter and Rosselló-Móra (2009), *Shifting the genomic gold standard for the prokaryotic species definition*. [10.1073/pnas.0906412106](https://doi.org/10.1073/pnas.0906412106) | Relevant primary PMC full-text sections returned by search: ANIb and ANIm; Database Information; BLAST/MUMmer Calculation of ANI | ANIm is described using delta similarity errors and alignment lengths; no explicit ambiguity subtraction in inspected sections. Direct page access was challenged. |
| Lee et al. (2016), *OrthoANI: An improved algorithm and software for calculating average nucleotide identity*. [10.1099/ijsem.0.000760](https://doi.org/10.1099/ijsem.0.000760) | Publisher full PDF; original ANI and OrthoANI Methods, p. 1101; Fig. 1; Results and Discussion | Reciprocal fragment hits; short-fragment rejection; no N-rich threshold or per-column N policy found in these sections. |
| Jain et al. (2018), *High throughput ANI analysis of 90K prokaryotic genomes reveals clear species boundaries*. [10.1038/s41467-018-07641-9](https://doi.org/10.1038/s41467-018-07641-9) | PMC full text; Methods (Mashmap, FastANI extension, parameters); main-text ambiguity search | K-mer identity estimates and reciprocal mappings, not a base-column identity calculation. Supplement could not be fetched. |
| Shaw and Yu (2023), *Fast and robust metagenomic sequence comparison through sparse chaining with skani*. [10.1038/s41592-023-02018-3](https://doi.org/10.1038/s41592-023-02018-3) | Publisher full text; sequence identity, chains, AF, regression methods; Supplementary Note and Table 1; ambiguity search of supplement | Estimates identity through local k-mer matching; ANIm calibration. No explicit general N-column policy found in inspected publication material. |
| Larralde, Zeller and Carroll (2025), *PyOrthoANI, PyFastANI, and Pyskani: a suite of Python libraries for computation of average nucleotide identity*. [10.1093/nargab/lqaf095](https://doi.org/10.1093/nargab/lqaf095) | PMC full text, Materials and methods; original implementation statement | Explicit rejection of fragments with >80% Ns; no per-column exclusion formula specified. |
| LASTZ, [official manual](https://lastz.github.io/lastz/) (living software documentation, accessed 2026-10-09) | Back-end Filtering; Non-ACGT Characters, Splicing, and Separation; ambiguous scoring options | Explicit non-ACGT exclusion from reported identity, separately defined coverage and alignment scoring. |
| dRep, [official algorithm overview](https://drep.readthedocs.io/en/latest/choosing_parameters.html#overview-of-genome-comparison-algorithms) (living documentation) | Overview of genome comparison algorithms, ANIn/ANImf discussion | ANImf filters competing/repeat alignments; this is not a documented ambiguous-column mask. |
| pyANI-plus, [official walkthrough](https://pyani-plus.github.io/pyani-plus-docs/walkthrough.html) and [repository](https://github.com/pyani-plus/pyani-plus) (living documentation) | ANIm/ANIb walkthrough and citation statement | NUCmer modes and external methods; no ambiguity-specific policy found in inspected documentation. Source parser retrieval was unsuccessful. |

## What the papers actually establish

### Early fragment ANI and ANIm

Goris et al. fragment the query into 1020-nt pieces, retain best BLASTN matches, and average identities for eligible homologous matches. Their original eligibility text gives >30% overall identity and at least 70% alignable length; this should not silently be replaced by the different thresholds used in later accounts. Conserved DNA is a separate statistic: summed aligned lengths above 90% identity divided by total query genomic length. The stated motivation is mimicking DDH fragmentation and avoiding unreliable homology inference, not an ambiguity-base correction. [Publisher PDF, Methods](https://www.microbiologyresearch.org/content/journal/ijsem/10.1099/ijs.0.64483-0?crawler=true&mimetype=application/pdf).

Richter and Rosselló-Móra implement ANIb following Goris, and ANIm using NUCmer's delta output. They describe subtracting **similarity errors** from alignment length and combining alignments. The brief extract is: “the number of similarity errors was subtracted from the alignment length.” That description alone does not resolve N-to-N treatment, distinguish all delta error fields, or establish exclusion from the denominator. The article's main argument is efficient genome-based species circumscription; the inspected Methods do not motivate ambiguity masking. [Primary full text, MUMmer Calculation of ANI](https://pmc.ncbi.nlm.nih.gov/articles/PMC2776425/).

### OrthoANI and the explicit N-rich-fragment rule

The 2016 OrthoANI methods fragment both genomes, discard pieces shorter than 1020 bp, identify reciprocal best BLAST hits covering at least 35% of a fragment, and average fragment-pair identities. Its stated motivation is resolving reciprocal ANI asymmetry. The inspected original methods do not state an N-rich-fragment filter. [Publisher PDF, OrthoANI algorithm and Discussion](https://www.microbiologyresearch.org/content/journal/ijsem/10.1099/ijsem.0.000760?crawler=true&mimetype=application/pdf).

The 2025 PyOrthoANI paper explicitly adds this description of the implemented algorithm: “contain >80% ambiguous (N) nucleotides are discarded.” It says the Python implementation follows the original Java implementation. This is strong published evidence that an ANI implementation deliberately excludes highly ambiguous fragments. It is not evidence that every N-bearing fragment is removed, that remaining N-to-N columns are matches, or that Ns are subtracted from individual alignment denominators. The text documents the rule but does not separately explain the statistical motivation for the 80% threshold; attributing a specific bias rationale to the authors would go beyond that text. [Materials and methods](https://pmc.ncbi.nlm.nih.gov/articles/PMC12246781/#sec2).

### FastANI and skani

FastANI derives fragment identity estimates from Mashmap's k-mer Jaccard model and averages reciprocal mapping estimates. Its published reliability condition is a minimum number of reciprocal mappings, not a minimum count of known/known aligned columns. This is a different estimator and supplies no direct precedent for a proposed ANIm denominator mask. The [official README](https://github.com/ParBLiSS/FastANI#usage-summary) defines query alignment fraction as reciprocal mapping count divided by total query fragments, again a different coverage quantity. [Published Methods](https://pmc.ncbi.nlm.nih.gov/articles/PMC6269478/).

skani estimates identity from locally chained k-mer matches and estimates aligned fraction separately; it also trains a regression adjustment against ANIm on MAG comparisons. Its argument concerns robustness to fragmentation and incompleteness. Agreement with an ANIm target is not evidence that the method shares a specific ambiguous-column formula. Supplementary Table 1 identifies its benchmark ANIm implementation as pyani v0.2.12. [Published Methods](https://www.nature.com/articles/s41592-023-02018-3), [Supplement, Table 1](https://media.springernature.com/original/springer-static/esm/art:10.1038%2Fs41592-023-02018-3/MediaObjects/41592_2023_2018_MOESM1_ESM.pdf).

## First-party implementation and adjacent method evidence

**LASTZ is adjacent aligner precedent, rather than a bacterial ANI publication.** Its identity excludes gaps and non-ACGT columns; coverage uses aligned input coordinates. Its gap policy is a separate choice. [Back-end Filtering](https://lastz.github.io/lastz/#back-end-filtering).

**skani source contains explicit N-aware seed suppression.** The inspected current `fmh_seeds` nucleotide function sets a resume position to `i + k` on uppercase or lowercase N and guards seed insertion with that position. This demonstrates ambiguity-aware seeding in that code path, not a pairwise base-column mask. This is implementation inference from a mutable `main` branch, not a statement in the 2023 paper or a verified behavior of every optimized path/IUPAC character. [Source, `fmh_seeds`, especially lines 251–279](https://github.com/bluenote-1577/skani/blob/main/src/seeding.rs#L251), [ASCII N constants](https://github.com/bluenote-1577/skani/blob/main/src/params.rs#L56).

**Do not assume all k-mer ANI tools discard ambiguous k-mers.** In the inspected FastANI v1.34 `addMinimizers` routine, sequence substrings are hashed literally; the reverse-complement switch leaves non-ACGT characters unchanged. No explicit ambiguous-base exclusion appears in that function. This narrow code observation does not establish full-program behavior, the quality of N-rich mappings, or historical behavior at publication. [Versioned source](https://github.com/ParBLiSS/FastANI/blob/v1.34/src/map/include/commonFunc.hpp#L34).

**ANImf's “filter” is a different operation.** dRep documents it as restricting competing alignments so a region aligns to one region of the other genome, improving results for repeats. The documentation also warns that nonhomologous repeat alignments can lower ANI, including self-comparisons. Neither statement documents per-column ambiguity exclusion. [Official overview](https://drep.readthedocs.io/en/latest/choosing_parameters.html#overview-of-genome-comparison-algorithms).

**pyANI-plus is not additional published evidence for an N-column correction here.** Its inspected walkthrough documents NUCmer-based ANIm and separate ANIb/dnadiff/FastANI methods; its repository currently asks users to cite Pritchard et al. (2016), *Genomics and taxonomy in diagnostics for food security: soft-rotting enterobacterial plant pathogens*, [10.1039/C5AY02550H](https://doi.org/10.1039/C5AY02550H). This check did not successfully retrieve its current delta parser, so it cannot establish that parser's ambiguity semantics. [Walkthrough](https://pyani-plus.github.io/pyani-plus-docs/walkthrough.html), [Citation statement](https://github.com/pyani-plus/pyani-plus#citing-pyani-plus).

One apparent search hit was excluded as ANI precedent after reading the full Methods: *A high-resolution genomic composition-based method with the ability to distinguish similar bacterial organisms* (2019), [10.1186/s12864-019-6119-x](https://doi.org/10.1186/s12864-019-6119-x). Its explicit removal of non-ACGT nucleotides belongs to the **oligonucleotide-frequency** calculation; its ANI/PSG section separately uses NUCmer. Combining those nearby paragraphs would falsely attribute ambiguity removal to ANI. [Methods sections](https://link.springer.com/article/10.1186/s12864-019-6119-x#Sec12).

## Interpreting the proposed identity and coverage choices

The following is our methodological reasoning, not a finding experimentally validated by these papers. If an N denotes missing information, N-to-N does not demonstrate that the underlying nucleotides agree, and N-to-A does not demonstrate that they differ. Excluding either kind of column estimates identity conditional on both nucleotides being known. It avoids awarding agreement for coincident unknown symbols and avoids treating uncertain information as demonstrated evolutionary divergence.

That choice changes the estimand. Write the identity rule explicitly, e.g. matching known/known columns divided by all eligible known/known columns, and specify gaps and IUPAC codes independently. Merely removing ambiguous matches from the numerator while leaving them in the denominator is a different statistic. If no eligible observations remain, identity is undefined; a reported numeric identity would require an additional convention.

Coverage need not use the identity denominator. Coordinate coverage answers how much input sequence lies in accepted aligned spans. Known-base coverage answers how much usable sequence information is compared. Changing identity does not, by itself, justify changing coverage. A future Terobacter definition should state both coverage numerator and genome-length denominator, including treatment of Ns.

## Search record and limitations

Searches used web search plus primary publisher/PMC full texts and first-party documentation/source. Representative queries, all on 2026-10-09:

- `Goris 2007 DNA DNA hybridization values genome sequence similarities average nucleotide identity ambiguous bases`
- `Richter Rossello Mora 2009 shifting genomic gold standard prokaryotic species definition ANI full text`
- `"OrthoANI" "2016" DOI`
- `"High throughput ANI analysis" "PMC" Jain 2018`
- `FastANI skani ambiguous N nucleotide identity genome N masking`
- `"PyOrthoANI" "ambiguous"`
- `"ANI" "ambiguous" "denominator"`
- `"average nucleotide identity" "non-ACGT"`
- `"average nucleotide identity" "unknown bases"`
- `"ANImf" "ambiguous"`; `"ANImf" "method" dRep`
- `"pyani-plus" "ambiguous"`
- `site:github.com/ParBLiSS/FastANI "ambiguous"`
- `site:github.com/bluenote-1577/skani "ambiguous"`

Text searches for `ambigu` and/or `unknown` supplemented reading the Methods; absence of a keyword is not absence of behavior. Many generic search hits concerned upstream read filtering, ambiguous taxonomy, or other metrics and were not treated as ANI-method evidence.

Access limits: Richter's directly opened PMC page returned a browser challenge and PNAS access failed, but search returned its primary Methods and relevant Results text; the supplement was not inspected. FastANI's supplement retrieval failed. OrthoANI's supplementary tables and PyOrthoANI's supplementary archive were not inspected. skani's supplementary note, methods table, and searchable supplement text were checked; no exhaustive image-by-image audit was performed. Historical versions and complete execution paths of the tool sources were not audited here. Mutable documentation/source links describe what was readable on the check date. These limits preclude a universal absence claim or a claim that all tools implement the same ambiguous-base policy.

## MUMmer papers, author explanation, and implementation history

### What the papers say

Kurtz et al. (2004), *Versatile and open software for comparing large genomes*, describes MUMmer3's exact-match anchors, clustering, and alignment extension. Its main text does not explicitly justify N-to-N identity counting or an ANI-specific ambiguity denominator. It is a general genome-alignment paper, rather than a calibration of ambiguity handling for ANI. The full main text was inspected using the author-hosted PDF. [10.1186/gb-2004-5-2-r12](https://doi.org/10.1186/gb-2004-5-2-r12), [primary PDF](https://mummer4.github.io/publications/MUMmer3.pdf).

Marçais et al. (2018), *MUMmer4: A fast and versatile genome alignment system*, emphasizes performance, concurrency and integration, and says nucmer4 produces the same output as nucmer3 apart from ordering. Its main text does not explain reference-N random substitution or justify a different ambiguity identity rule. The relevant implementation change was introduced in 2020, after this paper was published on 26 January 2018. The full article main text was inspected; its DOCX supplement was not successfully retrieved. [10.1371/journal.pcbi.1005944](https://doi.org/10.1371/journal.pcbi.1005944).

### Three distinct operations: seeds, alignment scores, and reported identity

The inspected pinned MUMmer3 executable calls the exact-match stage with `-n`, restricting nucleotide matches to A/C/G/T. This prevents unknown symbols from supplying exact-match anchors; it does not mean an extended alignment cannot span Ns. The official NUCmer documentation also describes A/C/G/T-only matching. [Official NUCmer notes](https://github.com/mummer4/mummer/blob/master/docs/nucmer.README).

In issue #51, a user relays an explanation received from MUMmer coauthor Adam Phillippy. It distinguishes alignment scoring (canonical matches +3; comparisons involving N score -7) from literal-character identity reporting, where N-to-N is equal and N-to-A is unequal. Delta similarity errors instead count comparisons with non-positive match scores. The explanation says the identity/similarity distinction matters for protein alignments and recommends `show-coords -B` to expose both values for nucleotide output. This is a reported author explanation, not a direct maintainer-authored public response. [Issue #51, 24 April 2020 comment](https://github.com/mummer4/mummer/issues/51#issuecomment-619080793).

The inspected pinned MUMmer3 `postnuc.cc` agrees with that distinction: it increments similarity errors for non-positive scores and identity errors when the characters differ. Thus N-to-N can receive a negative alignment score without increasing identity errors. The source inspection supports the reported explanation in [issue #51](https://github.com/mummer4/mummer/issues/51#issuecomment-619080793).

Using similarity errors would penalize Ns; it would not exclude them from both numerator and denominator. Also, Richter and Rosselló-Móra's prose uses the term similarity errors, whereas Terobacter's frozen P1 calculation uses the fifth delta field, identity errors. Those are separate facts; this literature check does not redefine either metric or establish historical JSpecies parser behavior.

### An explicit rationale for MUMmer4's reference-N randomization

On **30 September 2020**, Aleksey Zimin committed reference-N substitution under the message **“fixed the feature where very long stretches of Ns aligned”**. The patch adds a function replacing reference lowercase Ns with randomly chosen canonical letters and invokes it while constructing the reference sequence. A follow-up on 1 October simplifies the replacement code. Commit metadata and patches were retrieved through GitHub's first-party API. [Introducing commit 017e5df](https://github.com/mummer4/mummer/commit/017e5dfda38a174df649fc7fcfbcc3113cefcc75), [clarifying commit 2968770](https://github.com/mummer4/mummer/commit/2968770101909c82dfb2d7f9334de3706b0d7bcc).

This supplies an explicit practical objective: preventing long N stretches from aligning. The interpretation that random substitution disrupts N-only exact-match seeds follows from the implementation; the commit message itself does not discuss an ANI estimator, prove random replacement optimal, or justify reference/query asymmetry. No scientific comparison of alternative N identity policies was identified in the two MUMmer papers or the inspected issue discussion.

This finding qualifies the earlier Terobacter maintenance prototype. Its removal of reference-N substitution also removes this workaround. Earlier successful fixtures establish their measured results, but do not demonstrate protection against long N-only anchors. Reintroducing that failure mode is a risk inferred from the upstream history, not a new measured failure. Any future backend proposal should address seed eligibility separately from identity accounting, and review long N runs before treating removal of substitution as sufficient. Previous result files and the backend decision remain unchanged.

LASTZ's manual additionally says Ns are always excluded from seeds, independently of ambiguous-base scoring options. This supports separating seeding from identity accounting. [Official non-ACGT discussion](https://lastz.github.io/lastz/#non-acgt-characters-splicing-and-separation).

Additional MUMmer searches on 2026-10-09 included `MUMmer3 ambiguous N identity`, `MUMmer4 replace_n_random_letter`, and repository issue/history queries. Issue #51 comments, the introducing commit, the next revision, and history for `src/umd/nucmer.cc` were inspected. This was a bounded source/history check; neither all MUMmer discussions nor every release was audited.
