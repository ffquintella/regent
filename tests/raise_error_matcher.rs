// End-to-end: a real module + spec driven through the embedded regent-plan
// DSL (no host Ruby, no gems), verifying both raise_error idioms parse and
// produce the right pass/fail verdicts.
use regent::tester::ArtichokeTestRunner;
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
        manifests.join("boom.pp"),
        "class boom {\n  fail('parameter foo is required')\n}\n",
    )
    .unwrap();
    fs::write(
        manifests.join("ok.pp"),
        "class ok {\n  notify { 'fine': }\n}\n",
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
fn raise_error_idioms_end_to_end() {
    let spec = r#"
require 'spec_helper'

describe 'boom' do
  it 'regex hit' do
    is_expected.to compile.and_raise_error(/parameter foo is required/)
  end
  it 'regex miss' do
    is_expected.to compile.and_raise_error(/this text is absent/)
  end
  it 'block form matches' do
    expect { catalogue }.to raise_error(Puppet::Error, /parameter foo/)
  end
end

describe 'ok' do
  it 'compiles' do
    is_expected.to compile
  end
  it 'does not raise' do
    expect { catalogue }.not_to raise_error
  end
  it 'wrongly expects a raise' do
    expect { catalogue }.to raise_error(/should not happen/)
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
                panic!(
                    "no test case containing {needle:?}; have {:?}",
                    cases.keys()
                )
            })
    };

    use regent::tester::TestStatus;

    // boom: matching regex passes; non-matching fails; block form passes
    assert_eq!(find("regex hit").status, TestStatus::Passed);
    assert_eq!(find("regex miss").status, TestStatus::Failed);
    assert_eq!(find("block form matches").status, TestStatus::Passed);

    // ok: compiles; not_to raise passes; expecting a raise on clean compile fails
    assert_eq!(find("compiles").status, TestStatus::Passed);
    assert_eq!(find("does not raise").status, TestStatus::Passed);
    assert_eq!(find("wrongly expects a raise").status, TestStatus::Failed);
}
