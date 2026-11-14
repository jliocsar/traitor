//! Tests for the JSON store's names.

use super::*;

#[test]
fn a_name_is_the_lowercase_hex_of_its_sha256() {
    assert_eq!(
        hex_sha256(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}
