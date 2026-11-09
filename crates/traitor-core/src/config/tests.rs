use std::assert_matches;
use std::fs;

use super::*;

const COMMITTED_SCHEMA: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schema/traitor.schema.json"
);

fn config_json_schema() -> String {
    let schema = schemars::generate::SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<ConfigFile>();
    let mut text = serde_json::to_string_pretty(&schema).unwrap();
    text.push('\n');
    text
}

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
    assert_matches!(
        Config::parse(Path::new(CONFIG_PATH), contents),
        Err(ConfigError::MissingReason { .. })
    );
}
