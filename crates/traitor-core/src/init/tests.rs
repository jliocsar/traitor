use std::process::Command;

use super::*;

fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(directory.path())
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .unwrap();
    assert!(status.success());
    directory
}

fn version(text: &str) -> Version {
    Version::parse(text).unwrap()
}

fn write(directory: &Path, contents: &str) {
    let path = directory.join(CONFIG_PATH);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn read(directory: &Path) -> String {
    fs::read_to_string(directory.join(CONFIG_PATH)).unwrap()
}

#[test]
fn creates_a_config_pinned_to_the_binary() {
    let directory = repository();
    let outcome = init(directory.path(), &version("0.3.0")).unwrap();
    assert_eq!(outcome, InitOutcome::Created);
    assert_eq!(
        read(directory.path()),
        "{\n  \"$schema\": \"https://raw.githubusercontent.com/jliocsar/traitor/v0.3.0/schema/traitor.schema.json\",\n  \"traits\": {}\n}\n"
    );
    Config::load(directory.path()).unwrap();
}

#[test]
fn writes_at_the_repository_root_from_a_subdirectory() {
    let directory = repository();
    fs::create_dir_all(directory.path().join("src/deep")).unwrap();
    init(&directory.path().join("src/deep"), &version("0.3.0")).unwrap();
    assert!(directory.path().join(CONFIG_PATH).is_file());
}

#[test]
fn upgrades_an_older_config_and_keeps_its_settings_in_order() {
    let directory = repository();
    write(
        directory.path(),
        &format!(
            "{{\"$schema\":\"{}\",\"traits\":{{\"zeta\":{{\"threshold\":0.8}},\"alpha\":{{\"exempt\":[{{\"paths\":[\"a/**\"],\"reason\":\"generated\"}}]}}}}}}",
            schema_url(&version("0.2.1"))
        ),
    );
    let outcome = init(directory.path(), &version("0.3.0")).unwrap();
    assert_eq!(
        outcome,
        InitOutcome::Upgraded {
            from: version("0.2.1")
        }
    );
    let contents = read(directory.path());
    assert!(contents.contains("/v0.3.0/"));
    assert!(contents.find("zeta").unwrap() < contents.find("alpha").unwrap());
}

#[test]
fn adds_a_missing_schema_first() {
    let directory = repository();
    write(directory.path(), "{\"traits\":{}}");
    let outcome = init(directory.path(), &version("0.3.0")).unwrap();
    assert_eq!(outcome, InitOutcome::AddedSchema);
    assert!(read(directory.path()).starts_with("{\n  \"$schema\": "));
}

#[test]
fn leaves_a_current_config_untouched() {
    let directory = repository();
    let contents = format!(
        "{{ \"$schema\": \"{}\", \"traits\": {{}} }}",
        schema_url(&version("0.3.0"))
    );
    write(directory.path(), &contents);
    let outcome = init(directory.path(), &version("0.3.0")).unwrap();
    assert_eq!(outcome, InitOutcome::AlreadyCurrent);
    assert_eq!(read(directory.path()), contents);
}

#[test]
fn refuses_a_config_newer_than_the_binary() {
    let directory = repository();
    write(
        directory.path(),
        &format!("{{\"$schema\":\"{}\"}}", schema_url(&version("0.4.0"))),
    );
    assert!(matches!(
        init(directory.path(), &version("0.3.0")),
        Err(InitError::NewerThanBinary { .. })
    ));
}

#[test]
fn refuses_a_schema_that_isnt_traitors() {
    let directory = repository();
    write(directory.path(), "{\"$schema\":\"./schema.json\"}");
    assert!(matches!(
        init(directory.path(), &version("0.3.0")),
        Err(InitError::UnrecognizedSchema { .. })
    ));
}

#[test]
fn refuses_to_write_an_upgrade_that_wouldnt_load() {
    let directory = repository();
    let contents = format!(
        "{{\"$schema\":\"{}\",\"traits\":{{\"x\":{{\"threshold\":2}}}}}}",
        schema_url(&version("0.2.0"))
    );
    write(directory.path(), &contents);
    assert!(matches!(
        init(directory.path(), &version("0.3.0")),
        Err(InitError::Config {
            source: ConfigError::ThresholdOutOfRange { .. }
        })
    ));
    assert_eq!(read(directory.path()), contents);
}
