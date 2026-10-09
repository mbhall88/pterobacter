use std::collections::BTreeMap;
use terobacter::anim::parse_delta;

fn malformed_record_headers_return_errors(block: &str) {
    use terobacter::anim::AnimError;
    let refs = BTreeMap::from([("r".into(), 100)]);
    let queries = BTreeMap::from([("q".into(), 100)]);
    for header in [
        "unknown q nope 100",
        "r unknown 100 nope",
        "unknown q 100 100",
        "r unknown 100 100",
        "r q nope 100",
        "r q 100 nope",
        "r q -1 100",
        "r q 100 -1",
        "r q 18446744073709551616 100",
        "r q 100 18446744073709551616",
        "r q 0 100",
        "r q 100 0",
        "r q 99 100",
        "r q 100 99",
    ] {
        let text = format!("r.fa q.fa\nNUCMER\n>{header}\n{block}");
        assert!(
            matches!(
                parse_delta(&text, &refs, &queries),
                Err(AnimError::Invalid(_))
            ),
            "malformed header accepted: {header}"
        );
    }
}

#[test]
fn malformed_record_headers_without_blocks_are_errors_not_no_alignment() {
    malformed_record_headers_return_errors("");
}

#[test]
fn malformed_record_headers_with_blocks_are_errors_without_panicking() {
    malformed_record_headers_return_errors("1 10 1 10 0 0 0\n0\n");
}

#[test]
fn span_identity_and_coverage_keep_overlap_reverse_and_singleton_semantics() {
    let lengths = BTreeMap::from([("r".to_owned(), 100)]);
    let queries = BTreeMap::from([("q".to_owned(), 100)]);
    let delta = "r.fa q.fa\nNUCMER\n>r q 100 100\n1 50 50 1 2 2 0\n0\n40 60 40 60 1 1 0\n0\n70 70 70 70 0 0 0\n0\n";
    let record = parse_delta(delta, &lengths, &queries).unwrap();
    assert_eq!((record.d, record.e, record.blocks), (144, 3, 3));
    assert_eq!((record.reference_covered, record.query_covered), (61, 61));
    assert!(!record.identity_exceeds(96, 1).unwrap());
}

#[test]
fn empty_delta_is_successful_no_alignment_and_malformed_blocks_are_errors() {
    use terobacter::anim::AlignmentStatus;
    let r = BTreeMap::from([("r".to_owned(), 100)]);
    let q = BTreeMap::from([("q".to_owned(), 100)]);
    let record = parse_delta("r.fa q.fa\nNUCMER\n", &r, &q).unwrap();
    assert_eq!(record.status, AlignmentStatus::NoAlignment);
    assert_eq!(record.identity().unwrap(), None);
    assert_eq!((record.reference_covered, record.query_covered), (0, 0));
    for block in [
        "0 10 1 10 0 0 0\n0\n",
        "1 101 1 100 0 0 0\n0\n",
        "1 10 1 10 11 11 0\n0\n",
        "1 10 1 10 0 0 0\n",
        "1 10 1 10 0 0 0\n11\n0\n",
    ] {
        assert!(parse_delta(&format!("r.fa q.fa\nNUCMER\n>r q 100 100\n{block}"), &r, &q).is_err());
    }
}

#[test]
fn identity_is_strict_coverage_is_inclusive_and_wide_products_are_checked() {
    use terobacter::anim::{AlignmentStatus, AnimError, Metrics};
    let record = Metrics {
        status: AlignmentStatus::Aligned,
        d: 100,
        e: 2,
        blocks: 1,
        reference_covered: 80,
        query_covered: 79,
        reference_length: 100,
        query_length: 100,
    };
    assert!(!record.identity_exceeds(96, 1).unwrap());
    assert!(record.identity_exceeds(9599, 100).unwrap());
    assert!(record.coverage_at_least(true, 80, 1).unwrap());
    assert!(!record.coverage_at_least(false, 80, 1).unwrap());
    let huge = Metrics {
        d: u64::MAX,
        e: 0,
        ..record
    };
    assert!(matches!(
        huge.identity_exceeds(u64::MAX, u64::MAX),
        Err(AnimError::Overflow)
    ));
    let lengths = BTreeMap::from([("r".to_owned(), u64::MAX)]);
    assert!(matches!(parse_delta("r.fa q.fa\nNUCMER\n>r r 18446744073709551615 18446744073709551615\n1 18446744073709551615 1 18446744073709551615 0 0 0\n0\n", &lengths,&lengths),Err(AnimError::Overflow)));
}

#[test]
fn frozen_p1_overlap_reverse_and_weighted_fixtures_retain_the_contract() {
    for (name, length, expected) in [
        ("overlap", 20, (42, 2, 15, 15)),
        ("reverse", 40, (20, 0, 10, 10)),
        ("weighted", 40, (33, 5, 16, 17)),
    ] {
        let text = std::fs::read_to_string(format!(
            "{}/tests/fixtures/anim/{name}.filter",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let record = parse_delta(
            &text,
            &BTreeMap::from([("R".into(), length)]),
            &BTreeMap::from([("Q".into(), length)]),
        )
        .unwrap();
        assert_eq!(
            (
                record.d,
                record.e,
                record.reference_covered,
                record.query_covered
            ),
            expected
        );
    }
}
