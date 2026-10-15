//! Embedded skani search with AVX2-compatible seeding and content-ordered orientation ties.
//!
//! This adapter owns I/O and measurement only. It does not assign taxa. Database
//! authentication belongs to the reference-package loader; opened database files
//! must remain immutable for the lifetime of the memory mapping.

use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use bincode::Options;
use rayon::prelude::*;
use serde::Serialize;
use skani::params::{CommandParams, Mode, SketchParams, MIN_LENGTH_CONTIG};
use skani::sketch_db::SketchDbReader;
use skani::types::{AniEstResult, SeedPosition, Sketch, TaggedIndex};
use skani::{chain, regression, screen, seeding};

use crate::evidence::Percent;

pub const ENGINE_VERSION: &str = "0.3.2";
pub const ENGINE_REVISION: &str = "22da4ae9e44b2e3e686c8e5c71fcd16cec2b9791";
pub const ENGINE_PROFILE: &str = "skani-0.3.2-avx2-content-order-dna-v2";

#[derive(Debug, thiserror::Error)]
#[error("skani measurement failed: {0}")]
pub struct SkaniError(String);

fn error(message: impl std::fmt::Display) -> SkaniError {
    SkaniError(message.to_string())
}

/// The primary reporting floor is distinct from the biological AF80 gate.
/// Genomovar comparisons retain all returned rows, without a result-count cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchSettings {
    Primary,
    Genomovar,
}

impl SearchSettings {
    pub const fn reporting_floor_percent(self) -> u32 {
        match self {
            Self::Primary => 15,
            Self::Genomovar => 0,
        }
    }

    pub const fn screen_percent(self) -> u32 {
        80
    }

    fn command_params(self) -> CommandParams {
        CommandParams {
            screen: false,
            screen_val: f64::from(self.screen_percent()) / 100.0,
            mode: Mode::Search,
            out_file_name: String::new(),
            ref_files: Vec::new(),
            query_files: Vec::new(),
            refs_are_sketch: true,
            queries_are_sketch: false,
            robust: false,
            median: false,
            sparse: false,
            full_matrix: false,
            diagonal: false,
            max_results: usize::MAX,
            individual_contig_q: false,
            individual_contig_r: false,
            min_aligned_frac: f64::from(self.reporting_floor_percent()) / 100.0,
            both_min_aligned_frac: -0.01,
            keep_refs: false,
            est_ci: false,
            learned_ani: true,
            detailed_out: false,
            distance: false,
            rescue_small: false,
            separate_sketches: false,
            short_header: false,
        }
    }
}

/// A per-call selection. No environment variable or global mutable state is used.
#[derive(Debug, Clone, Copy)]
pub enum SeedingMode {
    Auto,
    Portable,
}

impl SeedingMode {
    fn seed(self, seq: &[u8], params: &SketchParams, contig: u32, sketch: &mut Sketch) {
        #[cfg(target_arch = "x86_64")]
        if matches!(self, Self::Auto) && is_x86_feature_detected!("avx2") {
            // SAFETY: runtime detection establishes the AVX2 requirement. Input
            // lengths and parameters have already been checked by sketch_query.
            unsafe { skani::avx2_seeding::avx2_fmh_seeds(seq, params, contig, sketch, true) };
            return;
        }
        seeding::fmh_seeds(seq, params, contig, sketch, true);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkaniRow {
    pub reference_file: String,
    pub ani: Percent,
    pub af_reference: Percent,
    pub af_query: Percent,
    pub reference_contig: String,
    pub query_contig: String,
}

impl SkaniRow {
    fn from_estimate(result: AniEstResult) -> Result<Self, SkaniError> {
        fn emitted(fraction: f32) -> Result<Percent, SkaniError> {
            // Multiplication is f32, just as in the pinned native writer.
            Percent::parse(&format!("{:.2}", fraction * 100.0))
                .ok_or_else(|| error("engine returned a non-finite or out-of-range percentage"))
        }
        Ok(Self {
            reference_file: result.ref_file,
            ani: emitted(result.ani)?,
            af_reference: emitted(result.align_fraction_ref)?,
            af_query: emitted(result.align_fraction_query)?,
            reference_contig: result.ref_contig,
            query_contig: result.query_contig,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchReport {
    pub engine_version: &'static str,
    pub engine_revision: &'static str,
    pub engine_profile: &'static str,
    pub settings: SearchSettings,
    pub reporting_floor_percent: u32,
    pub screen_percent: u32,
    pub total_input_bases: usize,
    pub sketched_bases: usize,
    pub rows: Vec<SkaniRow>,
    /// Missing estimates are retained explicitly; they are never ANI zero.
    pub not_returned_reference_files: Vec<String>,
}

/// A consolidated v0.3.2 DNA database, loaded without skani's CLI exit paths.
pub struct SkaniDatabase {
    params: SketchParams,
    markers: Vec<Sketch>,
    reader: SketchDbReader,
}

impl SkaniDatabase {
    /// Open an authenticated, immutable database component. Reject malformed
    /// files and inconsistent reference metadata before any screening. Reference
    /// payloads are checked once and released; search still reads them from mmap.
    pub fn open(directory: &Path) -> Result<Self, SkaniError> {
        let file = File::open(directory.join("markers.bin")).map_err(error)?;
        let limit = file.metadata().map_err(error)?.len();
        let (params, markers): (SketchParams, Vec<Sketch>) = bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .with_limit(limit)
            .reject_trailing_bytes()
            .deserialize_from(BufReader::new(file))
            .map_err(error)?;
        if params.k != 15
            || params.c == 0
            || params.marker_c < params.c
            || params.use_aa
            || params.use_syncs
        {
            return Err(error(
                "expected a k15 DNA database with valid compression factors",
            ));
        }
        let reader = SketchDbReader::new(
            directory
                .to_str()
                .ok_or_else(|| error("database path is not UTF-8"))?,
        )
        .map_err(error)?;
        if markers.is_empty() || markers.len() != reader.len() {
            return Err(error(
                "marker/index reference counts disagree or database is empty",
            ));
        }
        let mut ids = HashSet::new();
        for (i, marker) in markers.iter().enumerate() {
            validate_profile(marker, &params)?;
            if marker.file_name.is_empty()
                || !ids.insert(&marker.file_name)
                || reader.file_name(i) != Some(marker.file_name.as_str())
            {
                return Err(reference_error(
                    marker,
                    "marker/index identities disagree or reference names are duplicated",
                ));
            }
            let (reference_params, reference) = reader
                .get_sketch(i)
                .map_err(|e| reference_error(marker, e))?;
            if reference_params != params {
                return Err(reference_error(marker, "sketch parameters disagree"));
            }
            validate_reference(&reference, marker, &params)?;
        }
        Ok(Self {
            params,
            markers,
            reader,
        })
    }

    /// The pinned settings of a search of this database, in the form
    /// `c<compression>_m<marker compression>_learnedANI_s<screen>_minAF<floor>_uncapped`.
    pub fn settings_label(&self, settings: SearchSettings) -> String {
        let ani = if regression::use_learned_ani(self.params.c, false, false, false) {
            "learnedANI"
        } else {
            "unlearnedANI"
        };
        format!(
            "c{}_m{}_{ani}_s{}_minAF{}_uncapped",
            self.params.c,
            self.params.marker_c,
            settings.screen_percent(),
            settings.reporting_floor_percent()
        )
    }

    pub fn search(
        &self,
        query: &Path,
        settings: SearchSettings,
        threads: usize,
        mode: SeedingMode,
    ) -> Result<SearchReport, SkaniError> {
        let bytes = std::fs::read(query).map_err(error)?;
        let label = query
            .to_str()
            .ok_or_else(|| error("query path is not UTF-8"))?;
        self.search_bytes(label, &bytes, settings, threads, mode)
    }

    /// Search the exact bytes a workflow has validated and hashed. All supplied
    /// records reach the pinned sketcher; its native 500-base floor is unchanged.
    /// Keep the original query label for provenance. The content-order profile does not use
    /// filenames to choose orientation. The label is never opened as a file by this method.
    pub fn search_bytes(
        &self,
        query_label: &str,
        bytes: &[u8],
        settings: SearchSettings,
        threads: usize,
        mode: SeedingMode,
    ) -> Result<SearchReport, SkaniError> {
        if threads == 0 {
            return Err(error("thread count must be positive"));
        }
        let (query, total_input_bases) = sketch_query(query_label, bytes, &self.params, mode)?;
        let cp = settings.command_params();
        let learned = regression::use_learned_ani(self.params.c, false, false, false);
        let model = regression::get_model(self.params.c, learned);
        // A local pool permits repeated and concurrent calls with different thread
        // counts without installing a global logger or global Rayon pool.
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .map_err(error)?;
        let results: Result<Vec<Option<SkaniRow>>, SkaniError> = pool.install(|| {
            self.markers
                .par_iter()
                .enumerate()
                .map(|(i, marker)| {
                    // Preserve upstream's query/ref argument order for screening.
                    if query.contigs.is_empty()
                        || !screen::check_markers_quickly(&query, marker, cp.screen_val, false)
                    {
                        return Ok(None);
                    }
                    let (_, reference) = self
                        .reader
                        .get_sketch(i)
                        .map_err(|e| reference_error(marker, e))?;
                    let map = chain::map_params_from_sketch(&reference, false, &cp, &model);
                    let result = chain::chain_seeds_content_order(&reference, &query, map);
                    if result.ani > 0.5 {
                        SkaniRow::from_estimate(result).map(Some)
                    } else {
                        Ok(None)
                    }
                })
                .collect()
        });
        let mut rows: Vec<_> = results?.into_iter().flatten().collect();
        rows.sort_by(|a, b| {
            b.ani
                .cmp(&a.ani)
                .then_with(|| a.reference_file.cmp(&b.reference_file))
        });
        let returned: HashSet<_> = rows.iter().map(|r| r.reference_file.as_str()).collect();
        let mut missing: Vec<_> = self
            .markers
            .iter()
            .filter(|r| !returned.contains(r.file_name.as_str()))
            .map(|r| r.file_name.clone())
            .collect();
        missing.sort();
        Ok(SearchReport {
            engine_version: ENGINE_VERSION,
            engine_revision: ENGINE_REVISION,
            engine_profile: ENGINE_PROFILE,
            settings,
            reporting_floor_percent: settings.reporting_floor_percent(),
            screen_percent: settings.screen_percent(),
            total_input_bases,
            sketched_bases: query.total_sequence_length,
            rows,
            not_returned_reference_files: missing,
        })
    }
}

fn reference_error(sketch: &Sketch, message: impl std::fmt::Display) -> SkaniError {
    error(format!("reference {:?}: {message}", sketch.file_name))
}

fn validate_profile(sketch: &Sketch, params: &SketchParams) -> Result<(), SkaniError> {
    // The pinned Sketch::new stores c in marker_c, even when params.marker_c
    // differs. Check the serialized upstream value without changing that format.
    if sketch.k != params.k
        || sketch.c != params.c
        || sketch.marker_c != params.c
        || sketch.amino_acid
    {
        return Err(reference_error(sketch, "incompatible DNA sketch profile"));
    }
    Ok(())
}

fn validate_reference(
    reference: &Sketch,
    marker: &Sketch,
    params: &SketchParams,
) -> Result<(), SkaniError> {
    validate_profile(reference, params)?;
    if reference.file_name != marker.file_name
        || reference.contigs != marker.contigs
        || reference.total_sequence_length != marker.total_sequence_length
        || reference.marker_seeds != marker.marker_seeds
        || reference.contigs.is_empty()
        || reference.contigs.len() >= 1 << 30
        || reference.contigs.len() != reference.contig_lengths.len()
    {
        return Err(reference_error(marker, "sketch/marker metadata disagree"));
    }
    let total = reference
        .contig_lengths
        .iter()
        .try_fold(0_usize, |sum, &len| {
            if (len as usize) < MIN_LENGTH_CONTIG {
                None
            } else {
                sum.checked_add(len as usize)
            }
        });
    if total != Some(reference.total_sequence_length) {
        return Err(reference_error(
            marker,
            "invalid contig lengths or total length",
        ));
    }
    let seeds = reference
        .kmer_seeds_k
        .as_ref()
        .ok_or_else(|| reference_error(marker, "missing seed positions"))?;
    let validate_position = |position: &SeedPosition| {
        let length = reference
            .contig_lengths
            .get(position.contig_index() as usize);
        if position.contig_index_canonical & 0x8000_0000 != 0
            || length.is_none_or(|&len| position.pos >= len)
            || (position.pos as usize) < params.k - 1
        {
            return Err(reference_error(
                marker,
                "seed position lies outside its contig",
            ));
        }
        Ok(())
    };
    for positions in &reference.multi_position_storage {
        if positions.len() < 2 {
            return Err(reference_error(
                marker,
                "multiple-position storage has fewer than two positions",
            ));
        }
        for position in positions {
            validate_position(position)?;
        }
    }
    for &tag in seeds.values() {
        if TaggedIndex::is_single(tag) {
            validate_position(&TaggedIndex::get_single(tag))?;
        } else {
            let index = usize::try_from(tag >> 1)
                .ok()
                .and_then(|i| reference.multi_position_storage.get(i));
            if index.is_none() {
                return Err(reference_error(
                    marker,
                    "packed seed index lies outside position storage",
                ));
            }
        }
    }
    Ok(())
}

fn sketch_query(
    query_label: &str,
    bytes: &[u8],
    params: &SketchParams,
    mode: SeedingMode,
) -> Result<(Sketch, usize), SkaniError> {
    let mut sketch = Sketch::new(
        params.marker_c,
        params.c,
        params.k,
        query_label.into(),
        false,
    );
    let mut reader =
        needletail::parse_fastx_reader(std::io::Cursor::new(bytes.to_vec())).map_err(error)?;
    let mut total = 0_usize;
    while let Some(record) = reader.next() {
        let record = record.map_err(error)?;
        if record.format() != needletail::parser::Format::Fasta {
            return Err(error("query must be an assembly FASTA, not FASTQ"));
        }
        let seq = record.seq();
        if !is_iupac_sequence(&seq) {
            return Err(error(
                "query contains an empty sequence or a non-IUPAC nucleotide",
            ));
        }
        total = total
            .checked_add(seq.len())
            .ok_or_else(|| error("assembly length overflow"))?;
        if seq.len() < MIN_LENGTH_CONTIG {
            continue;
        }
        let len =
            u32::try_from(seq.len()).map_err(|_| error("contig exceeds skani's position limit"))?;
        let contig = u32::try_from(sketch.contigs.len()).map_err(error)?;
        if contig >= 1 << 30 {
            return Err(error("assembly exceeds skani's contig limit"));
        }
        let id = std::str::from_utf8(record.id()).map_err(error)?;
        sketch.contigs.push(id.to_owned());
        sketch.contig_lengths.push(len);
        sketch.total_sequence_length += seq.len();
        mode.seed(&seq, params, contig, &mut sketch);
    }
    if total == 0 {
        return Err(error("assembly contains no sequence"));
    }
    Ok((sketch, total))
}

/// Shared input validity predicate; parsing and errors remain with each caller.
pub(crate) fn is_iupac_sequence(sequence: &[u8]) -> bool {
    !sequence.is_empty()
        && sequence
            .iter()
            .all(|base| b"ACGTURYSWKMBDHVNacgturyswkmbdhvn".contains(base))
}
