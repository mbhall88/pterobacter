//! Behavioural tests over a tiny synthetic scheme: every engine must give the same call on the same assembly.
use mlst_feasibility_probes::engines::{ungapped, Oriented};
use mlst_feasibility_probes::fasta::{read_assembly, Contig};
use mlst_feasibility_probes::hsp::revcomp;
use mlst_feasibility_probes::package::{Allele, Package, LOCI};
use mlst_feasibility_probes::rules::{call, Call, Params};
use std::collections::HashMap;
use std::io::Write;

fn random_seq(n: usize, seed: u64) -> Vec<u8> {
    let mut s = seed;
    (0..n).map(|_| { s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); b"ACGT"[(s >> 33) as usize % 4] }).collect()
}

/// Seven loci with two 300-base alleles each (ids 1, 2); the second differs from the first by one base.
fn scheme() -> Package {
    let mut alleles = Vec::new();
    for l in 0..7 {
        let a1 = random_seq(300, 100 + l as u64);
        let mut a2 = a1.clone();
        a2[150] = if a1[150] == b'A' { b'C' } else { b'A' };
        alleles.push(Allele { locus: l, id: "1".into(), seq: a1 });
        alleles.push(Allele { locus: l, id: "2".into(), seq: a2 });
    }
    Package { alleles, profiles: HashMap::from([("1/1/1/1/1/1/1".to_string(), "5".to_string())]) }
}

fn assembly(pkg: &Package, pieces: &[(usize, bool, Option<usize>)]) -> Vec<Contig> {
    // (allele index, reverse-complement?, truncate to n bases)
    let mut seq = random_seq(2000, 1);
    for (i, &(a, rev, trunc)) in pieces.iter().enumerate() {
        let mut s = pkg.alleles[a].seq.clone();
        if let Some(n) = trunc { s.truncate(n); }
        if rev { s = revcomp(&s); }
        seq.extend(s);
        seq.extend(random_seq(1500, 10 + i as u64));
    }
    vec![Contig { name: "c1".into(), seq }]
}

fn run_ungapped(pkg: &Package, contigs: &[Contig]) -> Call {
    let ori = Oriented::new(pkg);
    let hsps = ungapped::run(pkg, &ori, contigs);
    let names: Vec<String> = contigs.iter().map(|c| c.name.clone()).collect();
    let lens: Vec<usize> = contigs.iter().map(|c| c.seq.len()).collect();
    call(pkg, &names, &lens, hsps, &Params::default()).0
}

#[test]
fn exact_forward_and_reverse_complement_alleles_give_the_profile_st() {
    let pkg = scheme();
    let pieces: Vec<_> = (0..7).map(|l| (l * 2, l % 2 == 1, None)).collect();
    let c = run_ungapped(&pkg, &assembly(&pkg, &pieces));
    assert_eq!((c.st.as_str(), c.status.as_str(), c.score), ("5", "PERFECT", 100));
    assert_eq!(c.exact_locations.iter().filter(|e| e.strand == "minus").count(), 3);
}

#[test]
fn near_identical_second_allele_is_not_exact_and_loses_the_profile() {
    let pkg = scheme();
    // locus 0 carries allele 2: exact for allele 2, so the profile 2/1/1/... is unrecorded (NOVEL)
    let mut pieces: Vec<_> = (0..7).map(|l| (l * 2, false, None)).collect();
    pieces[0].0 = 1;
    let c = run_ungapped(&pkg, &assembly(&pkg, &pieces));
    assert_eq!((c.tokens[0].as_str(), c.st.as_str(), c.status.as_str()), ("2", "-", "NOVEL"));
}

#[test]
fn truncated_allele_is_partial_and_names_the_highest_id_among_equal_scores() {
    let pkg = scheme();
    let mut pieces: Vec<_> = (0..7).map(|l| (l * 2, false, None)).collect();
    pieces[0].2 = Some(140); // 140/300 < 50%: dropped
    assert_eq!(run_ungapped(&pkg, &assembly(&pkg, &pieces)).tokens[0], "-");
    pieces[0].2 = Some(149); // allele 1 and 2 are identical over the first 149 bases
    let c = run_ungapped(&pkg, &assembly(&pkg, &pieces));
    assert_eq!(c.tokens[0], "2?");
    assert_eq!((c.status.as_str(), c.score), ("OK", 83));
}

#[test]
fn locus_absent_from_the_assembly_is_missing() {
    let pkg = scheme();
    let pieces: Vec<_> = (1..7).map(|l| (l * 2, false, None)).collect();
    let c = run_ungapped(&pkg, &assembly(&pkg, &pieces));
    assert_eq!((c.tokens[0].as_str(), c.status.as_str(), c.score), ("-", "MISSING", 77));
}

#[cfg(feature = "engine-exact-bio")]
#[test]
fn exact_hash_route_agrees_with_the_ungapped_route_on_inexact_and_partial_loci() {
    use mlst_feasibility_probes::engines::exact::{run, Pairwise};
    let pkg = scheme();
    let ori = Oriented::new(&pkg);
    let mut pieces: Vec<_> = (0..7).map(|l| (l * 2, l == 3, None)).collect();
    pieces[0].2 = Some(149);
    pieces[1].2 = Some(200);
    let contigs = assembly(&pkg, &pieces);
    let (hsps, stats) = run(&pkg, &ori, &contigs, Pairwise::Bio);
    let names = vec!["c1".to_string()];
    let lens = vec![contigs[0].seq.len()];
    let via_exact = call(&pkg, &names, &lens, hsps, &Params::default()).0;
    let via_ungapped = run_ungapped(&pkg, &contigs);
    assert_eq!((&via_exact.tokens, &via_exact.st, &via_exact.status, via_exact.score), (&via_ungapped.tokens, &via_ungapped.st, &via_ungapped.status, via_ungapped.score));
    assert_eq!(stats.loci_without_exact, 2);
}

#[test]
fn assembly_validation_rejects_malformed_input_and_maps_ambiguity_to_n() {
    let dir = std::env::temp_dir().join(format!("mlst-probe-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let write = |name: &str, text: &str| { let p = dir.join(name); std::fs::File::create(&p).unwrap().write_all(text.as_bytes()).unwrap(); p };
    assert!(read_assembly(&write("empty.fa", "")).unwrap_err().contains("no FASTA records"));
    assert!(read_assembly(&write("nohdr.fa", "ACGT\n")).unwrap_err().contains("before first header"));
    assert!(read_assembly(&write("dup.fa", ">a\nACGT\n>a\nACGT\n")).unwrap_err().contains("duplicate"));
    assert!(read_assembly(&write("prot.fa", ">p\nMKVLE\n")).unwrap_err().contains("invalid nucleotide"));
    assert!(read_assembly(&write("emptyrec.fa", ">a\n>b\nACGT\n")).unwrap_err().contains("no sequence"));
    assert!(read_assembly(&dir.join("missing.fa")).is_err());
    let ok = read_assembly(&write("ok.fa", ">a desc\nacgRn\nT\n")).unwrap();
    assert_eq!((ok[0].name.as_str(), ok[0].seq.as_slice()), ("a", b"ACGNNT".as_slice()));
    let _ = LOCI;
}
