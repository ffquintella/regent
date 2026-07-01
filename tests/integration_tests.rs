use std::fs;
use std::path::Path;

#[test]
fn test_module_creation() {
    let test_dir = Path::new("/tmp/regent_test_module");
    if test_dir.exists() {
        fs::remove_dir_all(test_dir).unwrap();
    }

    // Placeholder: module-creation logic is covered by the CLI tests; this
    // stub just exercises the temp-dir cleanup path.
}

#[test]
fn test_validation() {
    // Placeholder: validation logic is covered by the validator unit tests.
}

#[test]
fn test_config_defaults() {
    let config = regent::Config::default();
    assert_eq!(config.license, Some("Apache-2.0".to_string()));
}
