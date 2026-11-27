//! Every manifest of the official Forge modules the docker-based modules depend
//! on must be accepted by the embedded Puppet parser. The copies under
//! `tests/fixtures/forge` are unmodified (puppetlabs/docker 10.5.0,
//! puppetlabs/apt 11.4.0, puppetlabs/stdlib 10.1.0, all Apache-2.0).

use regent::tester::puppet_eval::check_manifest_syntax;
use std::path::{Path, PathBuf};

fn pp_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            pp_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "pp") {
            out.push(path);
        }
    }
}

#[test]
fn forge_modules_parse_without_errors() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/forge");
    let mut files = Vec::new();
    pp_files(&root, &mut files);
    assert!(files.len() >= 40, "expected the fixture copies, got {}", files.len());
    let failures: Vec<String> = files
        .iter()
        .filter_map(|f| {
            let src = std::fs::read_to_string(f).unwrap();
            check_manifest_syntax(&src)
                .err()
                .map(|e| format!("{}:{e}", f.display()))
        })
        .collect();
    assert!(failures.is_empty(), "parse failures:\n{}", failures.join("\n"));
}

#[test]
fn parse_error_reports_line_column_and_snippet_not_byte_offset() {
    let src = "# header\n\nclass foo {\n  $x = 1\n}\n\nclass bar inherits {\n}\n";
    let msg = check_manifest_syntax(src).unwrap_err().to_string();
    assert!(msg.starts_with("7:"), "line:col expected, got: {msg}");
    assert!(msg.contains("near `class bar inherits {`"), "{msg}");
}
