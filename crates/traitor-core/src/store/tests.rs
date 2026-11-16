//! Tests for the JSON store's names.

use std::assert_matches;

use super::*;

#[test]
fn a_name_is_the_lowercase_hex_of_its_sha256() {
    assert_eq!(
        hex_sha256(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn a_write_that_fails_says_where() {
    let directory = tempfile::TempDir::new().unwrap();
    let file = directory.path().join("not-a-directory");

    std::fs::write(&file, "").unwrap();

    let error = JsonStore::at(file.clone()).write("name", &1).unwrap_err();

    assert!(
        error.to_string().starts_with(&format!(
            "couldn't write {}: ",
            file.join("name.json").display()
        )),
        "{error}"
    );
}

#[test]
fn a_store_without_a_directory_says_it_cant_write() {
    let store = JsonStore { directory: None };

    assert_matches!(store.write("name", &1), Err(CacheWriteError::NoDirectory));
    assert_eq!(store.read::<u8>("name"), None);
}
