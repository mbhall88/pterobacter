//! Published file pins and cache keys use lowercase, two-digit SHA-256 bytes.
use terobacter::anim::{hash_bytes, hash_file};

#[test]
fn sha256_known_vectors_match_for_bytes_and_streamed_files() {
    let cases = [
        (
            Vec::new(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            b"abc".to_vec(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            vec![b'a'; 1_000_000],
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
        ),
    ];
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input.bin");
    for (bytes, expected) in cases {
        assert_eq!(hash_bytes(&bytes), expected);
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(hash_file(&path).unwrap(), expected);
    }
}
