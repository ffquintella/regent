// End-to-end: a real module + spec driven through the embedded regent-plan
// DSL (no host Ruby, no gems), covering three rendering fixes:
//   1. EPP templates reading fully-qualified class variables (`$mod::var`) from
//      the calling scope render the real value, not `undef`.
//   2. `without_content(/re/)` is honored (it was previously a silent no-op).
//   3. `Sensitive(...).unwrap` inside EPP renders the wrapped value.
use regent::tester::{ArtichokeTestRunner, TestStatus};
use regent::{TestConfig, TestType};
use std::collections::HashMap;
use std::fs;

fn write_module(spec_body: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let manifests = dir.path().join("manifests");
    let templates = dir.path().join("templates");
    let spec_classes = dir.path().join("spec").join("classes");
    fs::create_dir_all(&manifests).unwrap();
    fs::create_dir_all(&templates).unwrap();
    fs::create_dir_all(&spec_classes).unwrap();

    fs::write(
        dir.path().join("metadata.json"),
        r#"{ "name": "author-bastionvault", "version": "0.1.0" }"#,
    )
    .unwrap();

    // A quadlet-style `.container` rendered from an EPP template that reads
    // class variables from scope and unwraps a Sensitive token.
    fs::write(
        manifests.join("init.pp"),
        "class bastionvault (\n\
        \x20 String $network = 'vault-net',\n\
        \x20 Integer $publish_port = 8200,\n\
        \x20 String $token = 'super-secret',\n\
        ) {\n\
        \x20 $wrapped = Sensitive($token)\n\
        \x20 file { '/etc/containers/systemd/vault.container':\n\
        \x20\x20\x20 content => epp('bastionvault/vault.container.epp'),\n\
        \x20 }\n\
        }\n",
    )
    .unwrap();
    fs::write(
        templates.join("vault.container.epp"),
        "[Container]\n\
        Network=<%= $bastionvault::network %>\n\
        PublishPort=<%= $bastionvault::publish_port %>:<%= $bastionvault::publish_port %>\n\
        Token=<%= $bastionvault::wrapped.unwrap %>\n",
    )
    .unwrap();

    fs::write(spec_classes.join("example_spec.rb"), spec_body).unwrap();
    dir
}

fn run(dir: &tempfile::TempDir) -> regent::TestResults {
    let config = TestConfig::new(dir.path(), TestType::Unit);
    ArtichokeTestRunner::new(&config).run_unit_tests().unwrap()
}

fn by_name(results: &regent::TestResults) -> HashMap<String, regent::tester::TestCase> {
    results
        .test_cases
        .iter()
        .map(|tc| (tc.name.clone(), tc.clone()))
        .collect()
}

#[test]
fn epp_scope_and_without_content_end_to_end() {
    let spec = r#"
require 'spec_helper'

describe 'bastionvault' do
  it 'renders scope variables instead of undef' do
    is_expected.to contain_file('/etc/containers/systemd/vault.container')
      .with_content(/Network=vault-net/)
      .with_content(/PublishPort=8200:8200/)
  end

  it 'unwraps a Sensitive token in the template' do
    is_expected.to contain_file('/etc/containers/systemd/vault.container')
      .with_content(/Token=super-secret/)
  end

  it 'without_content passes when the pattern is genuinely absent' do
    is_expected.to contain_file('/etc/containers/systemd/vault.container')
      .without_content(/Network=undef/)
  end

  it 'without_content fails when the pattern is present' do
    is_expected.to contain_file('/etc/containers/systemd/vault.container')
      .without_content(/\[Container\]/)
  end
end
"#;
    let module = write_module(spec);
    let results = run(&module);

    eprintln!("stderr:\n{}", results.stderr);
    for tc in &results.test_cases {
        eprintln!("[{:?}] {} :: {:?}", tc.status, tc.name, tc.message);
    }

    let cases = by_name(&results);
    let find = |needle: &str| {
        cases
            .iter()
            .find(|(name, _)| name.contains(needle))
            .map(|(_, tc)| tc.clone())
            .unwrap_or_else(|| {
                panic!("no test case containing {needle:?}; have {:?}", cases.keys())
            })
    };

    assert_eq!(
        find("renders scope variables").status,
        TestStatus::Passed,
        "scope vars should render: {:?}",
        find("renders scope variables").message
    );
    assert_eq!(
        find("unwraps a Sensitive token").status,
        TestStatus::Passed,
        "Sensitive#unwrap should render the value: {:?}",
        find("unwraps a Sensitive token").message
    );
    assert_eq!(
        find("genuinely absent").status,
        TestStatus::Passed,
        "without_content should pass when pattern absent: {:?}",
        find("genuinely absent").message
    );
    assert_eq!(
        find("pattern is present").status,
        TestStatus::Failed,
        "without_content must now fail when pattern present (was a silent no-op)"
    );
}
