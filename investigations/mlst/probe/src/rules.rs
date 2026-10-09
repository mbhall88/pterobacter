//! mlst 2.32.2's decision rules applied to a list of ungapped HSPs: BLAST output order, the identity and
//! coverage filters, `~`/`?`/exact tokens, ST lookup, score and STATUS. Engine independent.
use crate::hsp::{min_score_for_evalue, Hsp};
use crate::package::{Package, LOCI};
use serde::Serialize;
use std::collections::HashMap;

pub struct Params {
    pub minid: f64,
    pub mincov: f64,
    pub evalue: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params { minid: 95.0, mincov: 50.0, evalue: 1e-20 }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ExactLocation {
    pub locus: String,
    pub allele: String,
    pub contig: String,
    pub start: usize,
    pub end: usize,
    pub strand: &'static str,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Call {
    pub tokens: Vec<String>,
    #[serde(rename = "ST")]
    pub st: String,
    pub status: String,
    pub score: i64,
    pub exact_locations: Vec<ExactLocation>,
}

/// BLAST report order for one query batch: by contig (input order), then subject by its best HSP score
/// (descending), ties by database order (descending OID, as observed from the comparator), then HSP score.
pub fn blast_order(hsps: &mut [Hsp]) {
    let mut best: HashMap<(usize, usize), i32> = HashMap::new();
    for h in hsps.iter() {
        let e = best.entry((h.contig, h.allele)).or_insert(i32::MIN);
        *e = (*e).max(h.score);
    }
    hsps.sort_by(|a, b| {
        a.contig.cmp(&b.contig)
            .then(best[&(b.contig, b.allele)].cmp(&best[&(a.contig, a.allele)]))
            .then(b.allele.cmp(&a.allele))
            .then(b.score.cmp(&a.score))
            .then(a.start.cmp(&b.start))
    });
}

pub fn call(pkg: &Package, contig_names: &[String], contig_lens: &[usize], mut hsps: Vec<Hsp>, p: &Params) -> (Call, Vec<Hsp>) {
    let db_len = pkg.total_bases();
    hsps.retain(|h| {
        h.nident as f64 * 100.0 >= p.minid * h.alen as f64 && h.score >= min_score_for_evalue(contig_lens[h.contig], db_len, p.evalue)
    });
    blast_order(&mut hsps);
    let mut res: Vec<Option<String>> = vec![None; LOCI.len()];
    let mut exact = Vec::new();
    for h in &hsps {
        let a = &pkg.alleles[h.allele];
        let hlen = a.seq.len() as f64;
        if h.nident as f64 / hlen < p.mincov / 100.0 {
            continue;
        }
        if h.alen as usize == a.seq.len() && h.nident == h.alen {
            let slot = &mut res[a.locus];
            match slot {
                Some(cur) if !cur.contains('~') && !cur.contains('?') => cur.push_str(&format!(",{}", a.id)),
                _ => *slot = Some(a.id.clone()),
            }
            exact.push(ExactLocation { locus: LOCI[a.locus].into(), allele: a.id.clone(), contig: contig_names[h.contig].clone(),
                                       start: h.start + 1, end: h.end, strand: if h.minus { "minus" } else { "plus" } });
        } else if res[a.locus].is_none() {
            res[a.locus] = Some(if h.alen as usize == a.seq.len() { format!("~{}", a.id) } else { format!("{}?", a.id) });
        }
    }
    let mut tokens: Vec<String> = res.into_iter().map(|t| t.unwrap_or_else(|| "-".into())).collect();
    let sig = tokens.join("/");
    let mut st = pkg.profiles.get(&sig).cloned().unwrap_or_else(|| "-".into());
    let (c_tilde, c_q, c_dash) = (count(&sig, '~'), count(&sig, '?'), count(&sig, '-'));
    let (score, status) = score_and_status(&st, tokens.len(), c_tilde, c_q, c_dash, &tokens);
    // duplicates of pure numeric alleles are reported in numeric order
    for t in tokens.iter_mut() {
        if t.contains(',') && t.chars().all(|c| c.is_ascii_digit() || c == ',') {
            let mut v: Vec<u64> = t.split(',').map(|x| x.parse().unwrap()).collect();
            v.sort();
            *t = v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
        }
    }
    if st != "-" && sig.contains("/-") {
        // null-allele profiles: report 0, not -
        for t in tokens.iter_mut() {
            if t == "-" {
                *t = "0".into();
            }
        }
    }
    if st.is_empty() {
        st = "-".into();
    }
    (Call { tokens, st, status, score, exact_locations: exact }, hsps)
}

fn count(s: &str, c: char) -> usize {
    s.chars().filter(|&x| x == c).count()
}

/// Perl-compatible: `int(((n - .3 a) - .5 b - 1.0 c) * 90 / n) (+10 if ST)` and `status_column`.
pub fn score_and_status(st: &str, n: usize, tilde: usize, q: usize, dash: usize, tokens: &[String]) -> (i64, String) {
    let nl = n as f64;
    let mut s = nl;
    s -= 0.3 * tilde as f64;
    s -= 0.5 * q as f64;
    s -= 1.0 * dash as f64;
    let mut score = (s * 90.0 / nl).trunc() as i64;
    if st != "-" {
        score += 10;
    }
    let status = if st.chars().all(|c| c.is_ascii_digit()) && !st.is_empty() {
        "PERFECT"
    } else if score <= 0 {
        "NONE"
    } else if !tokens.iter().any(|t| t.chars().any(|c| !c.is_ascii_digit())) {
        "NOVEL"
    } else if tokens.iter().any(|t| t.contains(',')) {
        "MIXED"
    } else if tokens.iter().any(|t| t.contains('-')) {
        "MISSING"
    } else if score < 70 {
        "BAD"
    } else {
        "OK"
    };
    (score, status.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::Allele;

    fn pkg() -> Package {
        // two alleles per locus (ids 1, 2), 200 bases each, one profile (1/1/1/1/1/1/1 => ST7)
        let alleles = (0..7).flat_map(|l| (1..=2).map(move |i| Allele { locus: l, id: i.to_string(), seq: vec![b'A'; 200] })).collect();
        Package { alleles, profiles: HashMap::from([("1/1/1/1/1/1/1".to_string(), "7".to_string())]) }
    }

    fn hsp(locus: usize, id: usize, alen: u32, nident: u32, start: usize) -> Hsp {
        Hsp { contig: 0, allele: locus * 2 + id - 1, start, end: start + alen as usize, minus: false, nident, alen, score: nident as i32 - 2 * (alen - nident) as i32 }
    }

    fn run(h: Vec<Hsp>) -> Call {
        call(&pkg(), &["c".into()], &[100_000], h, &Params::default()).0
    }

    fn all_exact() -> Vec<Hsp> {
        (0..7).map(|l| hsp(l, 1, 200, 200, l * 1000)).collect()
    }

    #[test]
    fn exact_profile_gives_perfect_st() {
        let c = run(all_exact());
        assert_eq!((c.st.as_str(), c.status.as_str(), c.score), ("7", "PERFECT", 100));
    }

    #[test]
    fn missing_locus_is_dash_and_scores_one_locus_down() {
        let c = run(all_exact().into_iter().skip(1).collect());
        assert_eq!(c.tokens[0], "-");
        assert_eq!((c.st.as_str(), c.status.as_str(), c.score), ("-", "MISSING", 77));
    }

    #[test]
    fn inexact_full_length_is_tilde_and_partial_is_question() {
        let mut h = all_exact().into_iter().skip(2).collect::<Vec<_>>();
        h.push(hsp(0, 1, 200, 199, 0));
        h.push(hsp(1, 2, 150, 150, 1000));
        let c = run(h);
        assert_eq!(&c.tokens[..2], ["~1", "2?"]);
        assert_eq!((c.status.as_str(), c.score), ("OK", 79)); // int((7-.3-.5)*90/7); five `~` give 70, matching the comparator
    }

    #[test]
    fn exact_replaces_an_earlier_inexact_token_and_later_inexact_never_replaces_exact() {
        let mut h = all_exact().into_iter().skip(1).collect::<Vec<_>>();
        h.push(hsp(0, 2, 200, 199, 0)); // inexact, higher contig order position
        h.push(hsp(0, 1, 200, 200, 5000)); // exact later in the same contig
        assert_eq!(run(h).tokens[0], "1");
        let mut h = all_exact().into_iter().skip(1).collect::<Vec<_>>();
        h.push(hsp(0, 1, 200, 200, 0));
        h.push(hsp(0, 2, 200, 199, 5000));
        assert_eq!(run(h).tokens[0], "1");
    }

    #[test]
    fn two_exact_copies_make_a_duplicate_token_and_mixed_status() {
        let mut h = all_exact();
        h.push(hsp(0, 1, 200, 200, 9000));
        h.push(hsp(0, 2, 200, 200, 9500));
        let c = run(h);
        assert_eq!(c.tokens[0], "1,1,2");
        assert_eq!((c.st.as_str(), c.status.as_str()), ("-", "MIXED"));
        assert_eq!(c.exact_locations.len(), 9);
    }

    #[test]
    fn hits_below_half_identity_weighted_coverage_are_ignored() {
        let mut h = all_exact().into_iter().skip(1).collect::<Vec<_>>();
        h.push(hsp(0, 1, 99, 99, 0)); // 99/200 < 50%
        assert_eq!(run(h).tokens[0], "-");
    }

    #[test]
    fn hsps_below_95_percent_identity_are_dropped_before_the_rules_see_them() {
        let mut h = all_exact().into_iter().skip(1).collect::<Vec<_>>();
        h.push(hsp(0, 1, 200, 189, 0)); // 94.5%
        assert_eq!(run(h).tokens[0], "-");
    }

    #[test]
    fn first_inexact_in_blast_order_wins_highest_score_then_highest_oid() {
        let mut h = all_exact().into_iter().skip(1).collect::<Vec<_>>();
        h.push(hsp(0, 1, 200, 198, 0));
        h.push(hsp(0, 2, 200, 198, 0)); // same score: higher OID (allele 2) listed first
        assert_eq!(run(h).tokens[0], "~2");
        let mut h = all_exact().into_iter().skip(1).collect::<Vec<_>>();
        h.push(hsp(0, 1, 200, 199, 0));
        h.push(hsp(0, 2, 200, 198, 0));
        assert_eq!(run(h).tokens[0], "~1");
    }

    #[test]
    fn status_bad_needs_a_score_under_seventy_without_missing_or_mixed_tokens() {
        let tokens: Vec<String> = (0..7).map(|_| "~1".to_string()).collect();
        let (score, status) = score_and_status("-", 7, 7, 0, 0, &tokens);
        assert_eq!((score, status.as_str()), (63, "BAD"));
        let (score, status) = score_and_status("-", 7, 5, 0, 0, &tokens);
        assert_eq!((score, status.as_str()), (70, "OK"));
    }

    #[test]
    fn no_hits_is_none_with_zero_score() {
        let c = run(vec![]);
        assert_eq!((c.st.as_str(), c.status.as_str(), c.score), ("-", "NONE", 0));
    }

    #[test]
    fn exact_but_unrecorded_profile_is_novel() {
        let mut h = all_exact();
        h[0] = hsp(0, 2, 200, 200, 0);
        let c = run(h);
        assert_eq!((c.st.as_str(), c.status.as_str(), c.score), ("-", "NOVEL", 90));
    }
}
