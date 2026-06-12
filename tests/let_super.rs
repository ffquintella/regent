// End-to-end: a nested `context` overrides a `let` and calls `super()` to reach
// the enclosing context's same-named `let`, mirroring RSpec semantics. The
// embedded Artichoke evaluator cannot run `super()` inside an instance_eval'd
// block, so the test engine models each describe/context as a class in an
// inheritance chain and defines lets as methods — which is what makes `super()`
// resolve to the parent definition.
use regent::tester::{ArtichokeTestRunner, TestStatus};
use regent::{TestConfig, TestType};
use std::fs;

fn write_module(spec_body: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let manifests = dir.path().join("manifests");
    let spec_classes = dir.path().join("spec").join("classes");
    fs::create_dir_all(&manifests).unwrap();
    fs::create_dir_all(&spec_classes).unwrap();

    fs::write(
        manifests.join("myclass.pp"),
        "class myclass (String $mode = 'off') {\n  notify { \"mode-${mode}\": }\n}\n",
    )
    .unwrap();
    fs::write(spec_classes.join("myclass_spec.rb"), spec_body).unwrap();
    dir
}

#[test]
fn nested_let_can_call_super() {
    let spec = r#"
require 'spec_helper'

describe 'myclass' do
  let(:params) { { 'mode' => 'base' } }

  it 'uses the base params' do
    is_expected.to contain_notify('mode-base')
  end

  context 'overriding via super' do
    let(:params) { super().merge('mode' => 'override') }

    it 'merges over the parent let' do
      is_expected.to contain_notify('mode-override')
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

    assert!(
        !results.test_cases.is_empty(),
        "expected examples to be collected; super() in let must not abort plan building"
    );
    assert!(
        results
            .test_cases
            .iter()
            .all(|tc| tc.status == TestStatus::Passed),
        "all examples should pass: super() in the nested let must return the parent params hash"
    );
}
