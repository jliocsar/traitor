use std::fs;

use super::*;

const COMMITTED_SCHEMA: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schema/traitor.schema.json"
);

#[test]
fn committed_schema_is_current() {
    let generated = config_json_schema();
    if std::env::var_os("TRAITOR_WRITE_SCHEMA").is_some() {
        fs::write(COMMITTED_SCHEMA, &generated).unwrap();
    }
    let committed = fs::read_to_string(COMMITTED_SCHEMA).unwrap_or_default();
    assert!(
        committed == generated,
        "schema/traitor.schema.json is out of date. Run `TRAITOR_WRITE_SCHEMA=1 cargo test -p traitor-core committed_schema` and commit it."
    );
}

#[test]
fn rejects_a_blank_reason() {
    let contents = br#"{"traits":{"x":{"exempt":[{"paths":["a/**"],"reason":" "}]}}}"#;
    assert!(matches!(
        Config::parse(Path::new(CONFIG_PATH), contents),
        Err(ConfigError::MissingReason { .. })
    ));
}
