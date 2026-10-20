//! Published file pins and cache keys use lowercase, two-digit SHA-256 bytes.
#[path = "../src/hex.rs"]
mod hex;

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

#[test]
fn hex_encoder_preserves_zero_padding_and_all_byte_values() {
    let bytes: Vec<u8> = (0..=255).collect();
    let encoded = hex::encode(&bytes);
    assert_eq!(hex::encode(&[]), "");
    assert_eq!(encoded.len(), 512);
    for (byte, pair) in bytes.iter().zip(encoded.as_bytes().chunks_exact(2)) {
        let text = std::str::from_utf8(pair).unwrap();
        assert_eq!(u8::from_str_radix(text, 16).unwrap(), *byte);
        assert!(text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    }
    assert!(encoded.starts_with("000102030405060708090a0b0c0d0e0f"));
}
