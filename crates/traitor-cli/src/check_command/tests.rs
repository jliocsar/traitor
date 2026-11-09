use super::*;

#[test]
fn an_annotation_message_escapes_percent_signs_and_line_breaks() {
    assert_eq!(
        escape_annotation_message("100% of\r\nit: a, b"),
        "100%25 of%0D%0Ait: a, b"
    );
}

#[test]
fn an_annotation_property_also_escapes_colons_and_commas() {
    assert_eq!(
        escape_annotation_property("Railway, not guards: 100%"),
        "Railway%2C not guards%3A 100%25"
    );
}
