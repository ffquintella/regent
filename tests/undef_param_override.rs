// End-to-end: a `let(:params)` value of the bare symbol `:undef` is passed
// through to the catalog as an explicit Puppet `undef`, mirroring
// rspec-puppet/Puppet semantics:
//   1. On an `Optional` parameter, the explicit undef OVERRIDES the declared
//      default (the parameter reads back as undef, not its default value) — so a
//      negative test like `without_content(/plugin_dir/)` can be expressed.
//   2. On a non-`Optional` parameter, the explicit undef fails the type check
//      and compilation raises, the way real Puppet does.
// Regression guard: regent used to *drop* `:undef` keys, letting the default
// render and silently masking the negative test.
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
        r#"{ "name": "author-undefmod", "version": "0.1.0" }"#,
    )
    .unwrap();

    // `plugin_dir` is Optional with a non-undef default; the conf omits the
    // line entirely when it is undef (the common "if set, render it" pattern).
    fs::write(
        manifests.join("init.pp"),
        "class undefmod (\n\
        \x20 Optional[String] $plugin_dir = '/var/lib/default',\n\
        ) {\n\
        \x20 if $plugin_dir {\n\
        \x20\x20\x20 $line = \"plugin_dir=${plugin_dir}\\n\"\n\
        \x20 } else {\n\
        \x20\x20\x20 $line = ''\n\
        \x20 }\n\
        \x20 file { '/etc/undefmod.conf':\n\
        \x20\x20\x20 content => \"[main]\\n${line}\",\n\
        \x20 }\n\
        }\n",
    )
    .unwrap();

    // A non-Optional parameter: passing it `undef` must fail to compile.
    fs::write(
        manifests.join("strict.pp"),
        "class undefmod::strict (\n\
        \x20 String $name = 'svc',\n\
        ) {\n\
        \x20 notify { \"strict-${name}\": }\n\
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
fn undef_param_overrides_optional_default_and_rejects_non_optional() {
    let spec = r#"
require 'spec_helper'

describe 'undefmod' do
  context 'with defaults' do
    it 'renders the default plugin_dir' do
      is_expected.to contain_file('/etc/undefmod.conf')
        .with_content(%r{plugin_dir=/var/lib/default})
    end
  end

  context 'with plugin_dir => undef' do
    let(:params) { { plugin_dir: :undef } }

    it 'omits the plugin_dir line so undef overrides the default' do
      is_expected.to contain_file('/etc/undefmod.conf')
        .without_content(/plugin_dir/)
    end
  end
end

describe 'undefmod::strict' do
  context 'with name => undef' do
    let(:params) { { name: :undef } }

    it 'fails to compile because undef is not a String' do
      is_expected.to compile.and_raise_error(/expects a String value|expects a value/)
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
        find("renders the default").status,
        TestStatus::Passed,
        "default must render when no param passed: {:?}",
        find("renders the default").message
    );
    assert_eq!(
        find("omits the plugin_dir line").status,
        TestStatus::Passed,
        ":undef must override the Optional default so the line is absent: {:?}",
        find("omits the plugin_dir line").message
    );
    assert_eq!(
        find("fails to compile").status,
        TestStatus::Passed,
        ":undef on a non-Optional String must raise at compile time: {:?}",
        find("fails to compile").message
    );
}
