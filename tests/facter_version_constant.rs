// End-to-end regression for custom-fact specs that require Facter through the
// embedded Artichoke runner. Facter exposes FACTERVERSION as part of its public
// Ruby API, and facter 2.x reads it while loading `require 'facter'`.
use regent::tester::{ArtichokeTestRunner, TestStatus};
use regent::{TestConfig, TestType};
use std::fs;

#[test]
fn facter_version_constant_is_available_to_custom_fact_specs() {
    let module = tempfile::tempdir().unwrap();
    let manifests = module.path().join("manifests");
    let facts = module.path().join("lib").join("facter");
    let specs = module.path().join("spec").join("unit").join("facter");
    fs::create_dir_all(&manifests).unwrap();
    fs::create_dir_all(&facts).unwrap();
    fs::create_dir_all(&specs).unwrap();

    fs::write(
        module.path().join("metadata.json"),
        r#"{ "name": "author-facter_version", "version": "0.1.0" }"#,
    )
    .unwrap();
    fs::write(manifests.join("init.pp"), "class facter_version {}\n").unwrap();
    fs::write(
        facts.join("has_tool.rb"),
        "Facter.add(:has_tool) do\n  setcode { !Facter::Core::Execution.which('tool').nil? }\nend\n",
    )
    .unwrap();
    fs::write(
        specs.join("has_tool_spec.rb"),
        r#"
require 'facter'

describe :has_tool do
  before(:each) do
    Facter.clear
  end

  it 'loads Facter and evaluates the custom fact when the executable exists' do
    expect(Facter::Core::Execution).to receive(:which).with('tool').and_return('/usr/bin/tool')
    expect(Facter::FACTERVERSION).to eq('4.0.0')
    expect(Facter.value(:has_tool)).to eq(true)
  end

  it 'supports a missing executable result' do
    expect(Facter::Core::Execution).to receive(:which).with('tool').and_return(nil)
    expect(Facter.value(:has_tool)).to eq(false)
  end

  it 'restores the original method after a positive receive expectation' do
    expect(Facter.value(:has_tool)).to eq(true)
  end

  it 'passes a negative receive expectation when the method is absent' do
    expect(Facter::Core::Execution).not_to receive(:exec)
  end

  it 'fails a negative receive expectation when the method is invoked' do
    expect(Facter::Core::Execution).not_to receive(:which).with('tool')
    Facter.value(:has_tool)
  end

  it 'restores the original method after a failed receive expectation' do
    expect(Facter.value(:has_tool)).to eq(true)
  end
end
"#,
    )
    .unwrap();

    let config = TestConfig::new(module.path(), TestType::Unit);
    let results = ArtichokeTestRunner::new(&config).run_unit_tests().unwrap();

    assert_eq!(results.total, 6, "stderr: {}", results.stderr);
    assert_eq!(results.failed, 1, "stderr: {}", results.stderr);
    assert_eq!(results.passed, 5, "stderr: {}", results.stderr);

    let status = |needle: &str| {
        results
            .test_cases
            .iter()
            .find(|case| case.name.contains(needle))
            .map(|case| case.status.clone())
            .unwrap_or_else(|| panic!("missing case {needle:?}: {:?}", results.test_cases))
    };
    assert_eq!(
        status("passes a negative receive"),
        TestStatus::Passed,
        "stderr: {}",
        results.stderr
    );
    assert_eq!(
        status("fails a negative receive"),
        TestStatus::Failed,
        "stderr: {}",
        results.stderr
    );
    assert_eq!(
        status("after a positive receive"),
        TestStatus::Passed,
        "stderr: {}",
        results.stderr
    );
    assert_eq!(
        status("after a failed receive"),
        TestStatus::Passed,
        "stderr: {}",
        results.stderr
    );
}
