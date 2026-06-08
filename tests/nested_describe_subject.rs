// End-to-end: an rspec-puppet spec whose class under test is named only by the
// top-level `describe`, with nested `describe`/`context` blocks used purely as
// grouping labels. Regent must compile the top-level class (the rspec-puppet
// `top_level_description`), not a class literally named after a nested describe.
use regent::tester::ArtichokeTestRunner;
use regent::{TestConfig, TestType};
use std::fs;

fn write_module(spec_body: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let manifests = dir.path().join("manifests");
    let spec_classes = dir.path().join("spec").join("classes");
    fs::create_dir_all(&manifests).unwrap();
    fs::create_dir_all(&spec_classes).unwrap();

    fs::write(
        manifests.join("firewallmanager.pp"),
        "class firewallmanager {\n  notify { 'configured': }\n}\n",
    )
    .unwrap();
    fs::write(spec_classes.join("firewallmanager_spec.rb"), spec_body).unwrap();
    dir
}

#[test]
fn nested_describe_does_not_become_the_compiled_class() {
    let spec = r#"
require 'spec_helper'

describe 'firewallmanager' do
  context 'on RedHat 10' do
    describe 'on RedHat 10 (firewalld path)' do
      it 'declares the class' do
        is_expected.to contain_class('firewallmanager')
      end
      it 'compiles' do
        is_expected.to compile
      end
    end
  end
end
"#;
    let module = write_module(spec);
    let config = TestConfig::new(module.path(), TestType::Unit);
    let results = ArtichokeTestRunner::new(&config).run_unit_tests().unwrap();

    eprintln!("stderr:\n{}", results.stderr);
    for tc in &results.test_cases {
        eprintln!("[{:?}] {} :: {:?}", tc.status, tc.name, tc.message);
    }

    use regent::tester::TestStatus;
    // Both examples must pass: the subject resolves to the top-level
    // `firewallmanager`, so the class is found in the catalog and compiles.
    // Before the fix the nested describe label was compiled as the class,
    // which does not exist, failing both expectations.
    assert!(
        results
            .test_cases
            .iter()
            .all(|tc| tc.status == TestStatus::Passed),
        "all examples should pass when the top-level describe sets the subject"
    );
}
