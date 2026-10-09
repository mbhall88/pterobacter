//! Throwaway probe: `mlst-probe --engine E --package DIR --query FASTA [--preset P] [--hits-out F] [--raw-out F]`.
//! Prints one JSON record (call, timings, hit counts). Errors exit with status 2 and a message on stderr.
use mlst_feasibility_probes::engines::{self, Oriented};
use mlst_feasibility_probes::{fasta, package::Package, rules};
use serde_json::json;
use std::{env, path::PathBuf, process::exit, time::Instant};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let get = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let fail = |m: String| -> ! { eprintln!("error: {m}"); exit(2) };
    let engine = get("--engine").unwrap_or_else(|| fail("--engine required".into()));
    let pkg_dir = PathBuf::from(get("--package").unwrap_or_else(|| fail("--package required".into())));
    let query = PathBuf::from(get("--query").unwrap_or_else(|| fail("--query required".into())));
    let t0 = Instant::now();
    let pkg = Package::load(&pkg_dir).unwrap_or_else(|e| fail(e));
    let contigs = fasta::read_assembly(&query).unwrap_or_else(|e| fail(e));
    let ori = Oriented::new(&pkg);
    let t_setup = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    #[allow(unused_mut)]
    let mut raw = String::new();
    #[allow(unused_mut)]
    let mut extra = json!({});
    let hsps = match engine.as_str() {
        "ungapped" => engines::ungapped::run(&pkg, &ori, &contigs),
        #[cfg(feature = "engine-minimap2")]
        "minimap2" => {
            let preset = get("--preset").unwrap_or_else(|| "asm20".into());
            extra["preset"] = json!(preset);
            let via_file = get("--index").as_deref() == Some("file");
            extra["index"] = json!(if via_file { "file" } else { "in-memory" });
            engines::minimap2::run(&pkg, &ori, &contigs, &preset, via_file, Some(&mut raw)).unwrap_or_else(|e| fail(e))
        }
        #[cfg(feature = "engine-minimap2-pure")]
        "minimap2-pure" => {
            let preset = get("--preset").unwrap_or_else(|| "asm20".into());
            extra["preset"] = json!(preset);
            engines::minimap2_pure::run(&pkg, &ori, &contigs, &preset).unwrap_or_else(|e| fail(e))
        }
        #[cfg(feature = "engine-exact-bio")]
        "exact-bio" | "exact-only" => {
            let mode = if engine == "exact-only" { engines::exact::Pairwise::None } else { engines::exact::Pairwise::Bio };
            let (h, s) = engines::exact::run(&pkg, &ori, &contigs, mode);
            extra["stats"] = serde_json::to_value(s).unwrap();
            h
        }
        #[cfg(feature = "engine-exact-blockaligner")]
        "exact-blockaligner" => {
            let (h, s) = engines::exact::run(&pkg, &ori, &contigs, engines::exact::Pairwise::BlockAligner);
            extra["stats"] = serde_json::to_value(s).unwrap();
            h
        }
        other => fail(format!("engine {other} not compiled in")),
    };
    let t_search = t1.elapsed().as_secs_f64();
    let t2 = Instant::now();
    let names: Vec<String> = contigs.iter().map(|c| c.name.clone()).collect();
    let lens: Vec<usize> = contigs.iter().map(|c| c.seq.len()).collect();
    let (call, kept) = rules::call(&pkg, &names, &lens, hsps, &rules::Params::default());
    let t_rules = t2.elapsed().as_secs_f64();
    if let Some(p) = get("--hits-out") {
        let mut s = String::from("locus\tallele\tcontig\tstart\tend\tstrand\tnident\talen\thlen\n");
        for h in &kept {
            let a = &pkg.alleles[h.allele];
            s += &format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", mlst_feasibility_probes::package::LOCI[a.locus], a.id, names[h.contig], h.start + 1, h.end,
                          if h.minus { "minus" } else { "plus" }, h.nident, h.alen, a.seq.len());
        }
        std::fs::write(p, s).unwrap_or_else(|e| fail(e.to_string()));
    }
    if let Some(p) = get("--raw-out") {
        std::fs::write(p, raw).unwrap_or_else(|e| fail(e.to_string()));
    }
    let out = json!({ "engine": engine, "call": call, "hsps_after_filters": kept.len(),
                      "seconds": { "setup": t_setup, "search": t_search, "rules": t_rules }, "extra": extra });
    println!("{}", serde_json::to_string(&out).unwrap());
}
