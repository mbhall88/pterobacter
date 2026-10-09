use anim_feasibility_probes::read_fasta;
use minimap2::{Aligner, Preset, Strand};
use std::{env, path::Path, process};

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        return Err("usage: minimap2-probe asm5|asm10|asm20 REFERENCE QUERY".into());
    }
    let preset = match args[1].as_str() {
        "asm5" => Preset::Asm5,
        "asm10" => Preset::Asm10,
        "asm20" => Preset::Asm20,
        _ => return Err("unknown preset".into()),
    };
    read_fasta(Path::new(&args[2]))?;
    let queries = read_fasta(Path::new(&args[3]))?;
    let mut builder = Aligner::builder()
        .preset(preset)
        .with_cigar()
        .with_index_threads(1);
    builder.mapopt.flag |= minimap2_sys::MM_F_NO_PRINT_2ND as i64;
    let aligner = builder.with_index(&args[2], None).map_err(str::to_owned)?;
    for (name, sequence) in queries {
        let mappings = aligner
            .map(&sequence, false, false, None, None, Some(name.as_bytes()))
            .map_err(str::to_owned)?;
        for mapping in mappings {
            // The binding returns region records; do not rely on the output flag alone.
            if !(mapping.is_primary || mapping.is_supplementary) {
                continue;
            }
            let alignment = mapping.alignment.ok_or("missing base-level alignment")?;
            let cigar = alignment
                .cigar
                .ok_or("missing CIGAR")?
                .iter()
                .map(|(length, op)| {
                    let operation = match op {
                        0 => 'M',
                        1 => 'I',
                        2 => 'D',
                        7 => '=',
                        8 => 'X',
                        _ => panic!("unexpected assembly CIGAR operation"),
                    };
                    format!("{length}{operation}")
                })
                .collect::<String>();
            let target = mapping.target_name.ok_or("missing target name")?;
            let strand = if mapping.strand == Strand::Reverse {
                "-"
            } else {
                "+"
            };
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\tNM:i:{}\tcg:Z:{}",
                name,
                sequence.len(),
                mapping.query_start,
                mapping.query_end,
                strand,
                target,
                mapping.target_len,
                mapping.target_start,
                mapping.target_end,
                mapping.match_len,
                mapping.block_len,
                mapping.mapq,
                alignment.nm,
                cigar
            );
        }
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(2);
    }
}
