// End-to-end coverage for the relationship matchers, driven through the
// embedded regent-plan DSL (no host Ruby, no gems):
//   * `compile.with_all_deps` validates that every relationship reference
//     resolves to a declared resource (and fails when one dangles).
//   * `contain_*.that_requires / that_comes_before / that_notifies /
//     that_subscribes_to` validate against the catalog's dependency edges,
//     including the inverse-declaration forms.
use regent::tester::ArtichokeTestRunner;
use regent::{TestConfig, TestType};
use std::collections::HashMap;
use std::fs;

fn write_module(manifests: &[(&str, &str)], spec_body: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let manifests_dir = dir.path().join("manifests");
    let spec_classes = dir.path().join("spec").join("classes");
    fs::create_dir_all(&manifests_dir).unwrap();
    fs::create_dir_all(&spec_classes).unwrap();
    for (name, body) in manifests {
        fs::write(manifests_dir.join(name), body).unwrap();
    }
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

fn finder<'a>(
    cases: &'a HashMap<String, regent::tester::TestCase>,
) -> impl Fn(&str) -> regent::tester::TestCase + 'a {
    move |needle: &str| {
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
    }
}

#[test]
fn with_all_deps_validates_references() {
    use regent::tester::TestStatus;

    // `good` wires every relationship to a resource that exists.
    let good = "class good {\n  \
        package { 'p': ensure => installed }\n  \
        service { 's': ensure => running, require => Package['p'] }\n\
        }\n";
    // `dangling` points require at a resource that was never declared.
    let dangling = "class dangling {\n  \
        service { 's': ensure => running, require => Package['missing'] }\n\
        }\n";

    let spec = r#"
require 'spec_helper'

describe 'good' do
  it 'all deps resolve' do
    is_expected.to compile.with_all_deps
  end
end

describe 'dangling' do
  it 'has an unresolved dep' do
    is_expected.to compile.with_all_deps
  end
end
"#;
    let module = write_module(&[("good.pp", good), ("dangling.pp", dangling)], spec);
    let results = run(&module);
    for tc in &results.test_cases {
        eprintln!("[{:?}] {} :: {:?}", tc.status, tc.name, tc.message);
    }
    let cases = by_name(&results);
    let find = finder(&cases);

    assert_eq!(find("all deps resolve").status, TestStatus::Passed);
    let dangling = find("has an unresolved dep");
    assert_eq!(dangling.status, TestStatus::Failed);
    assert!(
        dangling
            .message
            .unwrap()
            .contains("unresolved dependencies"),
        "expected a dangling-dependency message"
    );
}

#[test]
fn relationship_matchers_check_the_graph() {
    use regent::tester::TestStatus;

    // a: service requires package directly; b: package declares before => service
    // (inverse form) and the service subscribes to a file it notifies back.
    let rel = "class rel {\n  \
        package { 'p': ensure => installed }\n  \
        file { '/etc/c': ensure => file }\n  \
        service { 's':\n    \
            ensure    => running,\n    \
            require   => Package['p'],\n    \
            subscribe => File['/etc/c'],\n  \
        }\n\
        }\n";

    let spec = r#"
require 'spec_helper'

describe 'rel' do
  it 'service requires package' do
    is_expected.to contain_service('s').that_requires('Package[p]')
  end
  it 'package comes before service (inverse of require)' do
    is_expected.to contain_package('p').that_comes_before('Service[s]')
  end
  it 'service subscribes to file' do
    is_expected.to contain_service('s').that_subscribes_to('File[/etc/c]')
  end
  it 'file notifies service (inverse of subscribe)' do
    is_expected.to contain_file('/etc/c').that_notifies('Service[s]')
  end
  it 'wrong direction fails' do
    is_expected.to contain_package('p').that_requires('Service[s]')
  end
  it 'missing relationship fails' do
    is_expected.to contain_service('s').that_notifies('Package[p]')
  end
end
"#;
    let module = write_module(&[("rel.pp", rel)], spec);
    let results = run(&module);
    for tc in &results.test_cases {
        eprintln!("[{:?}] {} :: {:?}", tc.status, tc.name, tc.message);
    }
    let cases = by_name(&results);
    let find = finder(&cases);

    assert_eq!(find("service requires package").status, TestStatus::Passed);
    assert_eq!(
        find("package comes before service").status,
        TestStatus::Passed
    );
    assert_eq!(
        find("service subscribes to file").status,
        TestStatus::Passed
    );
    assert_eq!(find("file notifies service").status, TestStatus::Passed);
    assert_eq!(find("wrong direction fails").status, TestStatus::Failed);
    assert_eq!(
        find("missing relationship fails").status,
        TestStatus::Failed
    );
}
