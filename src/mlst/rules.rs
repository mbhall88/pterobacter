//! The comparator's call rules applied to ungapped HSPs: report order, identity and coverage
//! filters, exact, `~` and `?` tokens, ST lookup, score and STATUS. Read from `mlst` 2.32.2 and
//! observed from its output; no `mlst` code is used.
use super::engine::{min_score_for_evalue, Hsp};
use super::scheme::{Scheme, LOCI};
use std::collections::HashMap;

pub(crate) const MIN_IDENTITY_PERCENT: u32 = 95;
pub(crate) const MIN_COVERAGE_PERCENT: u32 = 50;
pub(crate) const EVALUE: f64 = 1e-20;

/// One full-length identical hit, 1-based inclusive on the forward strand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExactHit {
    pub locus: usize,
    pub allele: usize,
    pub contig: usize,
    pub start: usize,
    pub end: usize,
    pub minus: bool,
}

#[derive(Debug, PartialEq)]
pub(crate) struct Call {
    pub tokens: Vec<String>,
    pub st: String,
    pub status: &'static str,
    pub score: i64,
    pub exact: Vec<ExactHit>,
}

/// Report order within one query: contig in input order, then allele by its best HSP score,
/// ties by later database position first, then HSP score.
fn report_order(hsps: &mut [Hsp]) {
    let mut best: HashMap<(usize, usize), i32> = HashMap::new();
    for h in hsps.iter() {
        let e = best.entry((h.contig, h.allele)).or_insert(i32::MIN);
        *e = (*e).max(h.score);
    }
    hsps.sort_by(|a, b| {
        a.contig
            .cmp(&b.contig)
            .then(best[&(b.contig, b.allele)].cmp(&best[&(a.contig, a.allele)]))
            .then(b.allele.cmp(&a.allele))
            .then(b.score.cmp(&a.score))
            .then(a.start.cmp(&b.start))
    });
}

pub(crate) fn call(scheme: &Scheme, contig_lens: &[usize], mut hsps: Vec<Hsp>) -> Call {
    let db = scheme.index.database_bases();
    hsps.retain(|h| {
        h.nident as u64 * 100 >= MIN_IDENTITY_PERCENT as u64 * h.alen as u64
            && h.score >= min_score_for_evalue(contig_lens[h.contig], db, EVALUE)
    });
    report_order(&mut hsps);
    let mut res: Vec<Option<String>> = vec![None; LOCI.len()];
    let mut exact = Vec::new();
    for h in &hsps {
        let a = &scheme.alleles[h.allele];
        let len = a.seq.len();
        if (h.nident as u64) * 100 < MIN_COVERAGE_PERCENT as u64 * len as u64 {
            continue;
        }
        if h.alen as usize == len && h.nident == h.alen {
            let slot = &mut res[a.locus];
            match slot {
                Some(cur) if !cur.contains('~') && !cur.contains('?') => {
                    cur.push(',');
                    cur.push_str(&a.id);
                }
                _ => *slot = Some(a.id.clone()),
            }
            exact.push(ExactHit {
                locus: a.locus,
                allele: h.allele,
                contig: h.contig,
                start: h.start + 1,
                end: h.end,
                minus: h.minus,
            });
        } else if res[a.locus].is_none() {
            res[a.locus] = Some(if h.alen as usize == len {
                format!("~{}", a.id)
            } else {
                format!("{}?", a.id)
            });
        }
    }
    let mut tokens: Vec<String> = res
        .into_iter()
        .map(|t| t.unwrap_or_else(|| "-".into()))
        .collect();
    let st = scheme
        .profiles
        .get(&tokens)
        .cloned()
        .unwrap_or_else(|| "-".into());
    let count = |c: char| tokens.iter().filter(|t| t.contains(c)).count();
    let (score, status) = score_and_status(&st, count('~'), count('?'), count('-'), &tokens);
    for t in &mut tokens {
        if t.contains(',') {
            let mut v: Vec<u64> = t.split(',').map(|x| x.parse().expect("digits")).collect();
            v.sort_unstable();
            *t = v.iter().map(u64::to_string).collect::<Vec<_>>().join(",");
        }
    }
    Call {
        tokens,
        st,
        status,
        score,
        exact,
    }
}

/// `int((n − .3 tilde − .5 partial − missing) · 90 / n) + 10 if ST` and the STATUS column.
pub(crate) fn score_and_status(
    st: &str,
    tilde: usize,
    partial: usize,
    missing: usize,
    tokens: &[String],
) -> (i64, &'static str) {
    let n = tokens.len() as f64;
    let s = n - 0.3 * tilde as f64 - 0.5 * partial as f64 - missing as f64;
    let mut score = (s * 90.0 / n).trunc() as i64;
    if st != "-" {
        score += 10;
    }
    let status = if st != "-" {
        "PERFECT"
    } else if score <= 0 {
        "NONE"
    } else if tokens.iter().all(|t| t.bytes().all(|b| b.is_ascii_digit())) {
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
    (score, status)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).into()).collect()
    }

    #[test]
    fn score_and_status_follow_the_comparator_table() {
        let exact = strings(&["1"; 7]);
        assert_eq!(score_and_status("7", 0, 0, 0, &exact), (100, "PERFECT"));
        assert_eq!(score_and_status("-", 0, 0, 0, &exact), (90, "NOVEL"));
        let mut missing = exact.clone();
        missing[0] = "-".into();
        assert_eq!(score_and_status("-", 0, 0, 1, &missing), (77, "MISSING"));
        let mut mixed = exact.clone();
        mixed[0] = "1,2".into();
        assert_eq!(score_and_status("-", 0, 0, 0, &mixed), (90, "MIXED"));
        let mut inexact = exact.clone();
        inexact[0] = "~1".into();
        inexact[1] = "2?".into();
        assert_eq!(score_and_status("-", 1, 1, 0, &inexact), (79, "OK"));
        let tilde = strings(&["~1"; 7]);
        assert_eq!(score_and_status("-", 7, 0, 0, &tilde), (63, "BAD"));
        assert_eq!(score_and_status("-", 5, 0, 0, &tilde), (70, "OK"));
        let none = strings(&["-"; 7]);
        assert_eq!(score_and_status("-", 0, 0, 7, &none), (0, "NONE"));
    }

    #[test]
    fn report_order_puts_best_scoring_allele_first_and_ties_to_the_later_allele() {
        let hsp = |allele, score| Hsp {
            contig: 0,
            allele,
            start: 0,
            end: 10,
            minus: false,
            nident: 10,
            alen: 10,
            score,
        };
        let mut hsps = vec![hsp(1, 5), hsp(2, 5), hsp(3, 9)];
        report_order(&mut hsps);
        assert_eq!(hsps.iter().map(|h| h.allele).collect::<Vec<_>>(), [3, 2, 1]);
    }
}
