//! Spec-runner behaviour that the docker-based modules rely on: `let` values
//! inside matcher arguments, shared examples with arguments, the `include(...)`
//! attribute matcher, clear errors for unsupported constructs, facts from
//! `on_supported_os` (structure + Debian codename) and `assert_type` with a
//! lambda.

use regent::tester::{ArtichokeTestRunner, TestStatus};
use regent::{TestConfig, TestType};
use std::fs;

fn run(manifest: &str, spec: &str) -> Vec<(String, TestStatus, Option<String>)> {
    let module = tempfile::tempdir().unwrap();
    fs::create_dir_all(module.path().join("manifests")).unwrap();
    fs::create_dir_all(module.path().join("spec/classes")).unwrap();
    fs::write(
        module.path().join("metadata.json"),
        r#"{ "name": "acme-demo", "version": "0.1.0" }"#,
    )
    .unwrap();
    fs::write(module.path().join("manifests/init.pp"), manifest).unwrap();
    fs::write(module.path().join("spec/classes/demo_spec.rb"), spec).unwrap();
    let config = TestConfig::new(module.path(), TestType::Unit)
        .with_pattern(Some("spec/classes/demo_spec.rb".to_string()));
    let results = ArtichokeTestRunner::new(&config).run_unit_tests().unwrap();
    results
        .test_cases
        .iter()
        .map(|c| (c.name.clone(), c.status.clone(), c.message.clone()))
        .collect()
}

fn all_passed(cases: &[(String, TestStatus, Option<String>)]) -> bool {
    !cases.is_empty() && cases.iter().all(|(_, s, _)| *s == TestStatus::Passed)
}

const MANIFEST: &str = r#"
class demo (Array $envs = ['A=1', 'B=2']) {
  notify { 'n': message => 'hello', tag => $envs }
  file { '/tmp/x': content => 'abc' }
}
"#;

#[test]
fn let_value_is_usable_inside_with_and_shared_examples_take_arguments() {
    let cases = run(
        MANIFEST,
        r#"
describe 'demo' do
  let(:expected_msg) { 'hello' }
  shared_examples 'has file' do |path|
    it { is_expected.to contain_file(path) }
  end
  it { is_expected.to contain_notify('n').with(message: expected_msg) }
  include_examples 'has file', '/tmp/x'
  it_behaves_like 'has file', '/tmp/x'
end
"#,
    );
    assert_eq!(cases.len(), 3, "{cases:?}");
    assert!(all_passed(&cases), "{cases:?}");
}

#[test]
fn include_matcher_checks_array_attributes() {
    let cases = run(
        MANIFEST,
        r#"
describe 'demo' do
  it { is_expected.to contain_notify('n').with(tag: include('B=2')) }
  it { is_expected.to contain_notify('n').with_tag(include('A=1', 'B=2')) }
  it { is_expected.to contain_notify('n').with(tag: include('Z=9')) }
end
"#,
    );
    assert_eq!(cases.len(), 3, "{cases:?}");
    assert_eq!(cases[0].1, TestStatus::Passed, "{cases:?}");
    assert_eq!(cases[1].1, TestStatus::Passed, "{cases:?}");
    assert_eq!(cases[2].1, TestStatus::Failed, "{cases:?}");
    assert!(
        cases[2].2.as_deref().unwrap_or("").contains("to include"),
        "{cases:?}"
    );
}

#[test]
fn unsupported_constructs_fail_with_a_clear_message() {
    let cases = run(
        MANIFEST,
        r#"
describe 'demo' do
  it { is_expected.to frobnicate_the_catalog }
end
"#,
    );
    let msg = cases[0].2.clone().unwrap_or_default();
    assert_eq!(cases[0].1, TestStatus::Failed);
    assert!(
        msg.contains("unsupported spec construct: 'frobnicate_the_catalog'"),
        "{msg}"
    );

    let undefined = run(
        MANIFEST,
        r#"
describe 'demo' do
  include_examples 'never declared'
  it { is_expected.to compile }
end
"#,
    );
    let all = format!("{undefined:?}");
    assert!(
        all.contains("shared examples/context 'never declared' is not defined"),
        "{all}"
    );
}

#[test]
fn on_supported_os_facts_are_nested_and_debian_has_a_codename() {
    let manifest = r#"
class demo {
  assert_type(Pattern[/\A(Debian|RedHat)\z/], $facts['os']['family']) |$a, $b| {
    fail("unsupported family ${b}")
  }
  $codename = $facts['os']['distro']['codename']
  notify { "${facts['os']['family']}-${codename}": }
}
"#;
    let cases = run(
        manifest,
        r#"
describe 'demo' do
  test_on = { supported_os: [{ 'operatingsystem' => 'Debian', 'operatingsystemrelease' => ['10'] }] }
  on_supported_os(test_on).each do |_os, os_facts|
    let(:facts) { os_facts }
    it { is_expected.to compile }
    it { is_expected.to contain_notify('Debian-buster') }
  end
end
"#,
    );
    assert_eq!(cases.len(), 2, "{cases:?}");
    assert!(all_passed(&cases), "{cases:?}");
}
