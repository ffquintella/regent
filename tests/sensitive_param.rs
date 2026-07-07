// End-to-end: rspec-puppet's `sensitive('secret')` helper works in Regent's
// embedded Artichoke runner, both as a `let(:params)` value and inside a
// `.with_*` matcher. Regent has no host Puppet/rspec-puppet, so the helper is
// provided by the runner's Ruby prelude; the wrapped value round-trips through
// the JSON plan tagged as `{"__sensitive__": ...}` and the Rust evaluator
// unwraps it transparently — matching how it already treats `Sensitive(...)`.
// Regression guard: a spec calling `sensitive(...)` used to abort on load with
// an undefined-method / uninitialized-constant error.
use regent::tester::{ArtichokeTestRunner, TestStatus};
use regent::{TestConfig, TestType};
use std::collections::HashMap;
use std::fs;

fn write_module(spec_body: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let manifests = dir.path().join("manifests");
    let spec_classes = dir.path().join("spec").join("classes");
    fs::create_dir_all(&manifests).unwrap();
    fs::create_dir_all(&spec_classes).unwrap();

    fs::write(
        dir.path().join("metadata.json"),
        r#"{ "name": "author-secretmod", "version": "0.1.0" }"#,
    )
    .unwrap();

    // A class with a Sensitive parameter that unwraps its value into a file's
    // content, the way a module rendering a credential would.
    fs::write(
        manifests.join("init.pp"),
        "class secretmod (\n\
        \x20 Sensitive[String] $password = Sensitive('s3cr3t'),\n\
        ) {\n\
        \x20 file { '/etc/secretmod.conf':\n\
        \x20\x20\x20 content => \"password=${password.unwrap}\\n\",\n\
        \x20 }\n\
        }\n",
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
fn sensitive_helper_works_in_params_and_matchers() {
    let spec = r#"
require 'spec_helper'

describe 'secretmod' do
  context 'with the default sensitive password' do
    it { is_expected.to compile }

    it 'renders the unwrapped default' do
      is_expected.to contain_file('/etc/secretmod.conf')
        .with_content(%r{password=s3cr3t})
    end
  end

  context 'with a sensitive param override' do
    let(:params) { { password: sensitive('hunter2') } }

    it 'accepts a sensitive() param and renders its unwrapped value' do
      is_expected.to contain_file('/etc/secretmod.conf')
        .with_content("password=hunter2\n")
    end

    it 'matches an expected value wrapped in sensitive()' do
      is_expected.to contain_file('/etc/secretmod.conf')
        .with_content(sensitive("password=hunter2\n"))
    end
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
        find("renders the unwrapped default").status,
        TestStatus::Passed,
        "default Sensitive param must unwrap in the catalog: {:?}",
        find("renders the unwrapped default").message
    );
    assert_eq!(
        find("accepts a sensitive() param").status,
        TestStatus::Passed,
        "sensitive() in let(:params) must pass through and unwrap: {:?}",
        find("accepts a sensitive() param").message
    );
    assert_eq!(
        find("matches an expected value wrapped").status,
        TestStatus::Passed,
        "sensitive() as a matcher's expected value must unwrap and compare: {:?}",
        find("matches an expected value wrapped").message
    );
}
