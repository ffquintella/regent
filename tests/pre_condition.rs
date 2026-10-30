use regent::tester::{ArtichokeTestRunner, TestStatus};
use regent::{TestConfig, TestType};
use std::fs;

#[test]
fn pre_condition_declares_parameterized_class_before_define_subject() {
    let module = tempfile::tempdir().unwrap();
    let manifests = module.path().join("manifests");
    let specs = module.path().join("spec/defines");
    fs::create_dir_all(&manifests).unwrap();
    fs::create_dir_all(&specs).unwrap();
    fs::write(
        module.path().join("metadata.json"),
        r#"{ "name": "example-web", "version": "0.1.0" }"#,
    )
    .unwrap();
    fs::write(
        manifests.join("init.pp"),
        r#"
class web($network = '') {
  if $network == '' {
    fail('Network must be defined')
  }
  $config_dir = "/srv/${network}"
}

define web::vhost() {
  if !defined(Class['web']) {
    include web
  }
  file { "${web::config_dir}/${title}.conf": ensure => file }
}
"#,
    )
    .unwrap();
    fs::write(
        specs.join("vhost_spec.rb"),
        r#"
describe 'web::vhost' do
  let(:title) { 'example' }
  let(:pre_condition) { "class { 'web': network => 'frontend' }" }

  it { is_expected.to compile }
  it { is_expected.to contain_file('/srv/frontend/example.conf') }
end
"#,
    )
    .unwrap();

    let config = TestConfig::new(module.path(), TestType::Unit)
        .with_pattern(Some("spec/defines/vhost_spec.rb".to_string()));
    let results = ArtichokeTestRunner::new(&config).run_unit_tests().unwrap();
    for case in &results.test_cases {
        eprintln!("[{:?}] {}: {:?}", case.status, case.name, case.message);
    }

    assert_eq!(results.total, 2);
    assert!(
        results
            .test_cases
            .iter()
            .all(|case| case.status == TestStatus::Passed),
        "pre_condition examples failed: {:?}",
        results.test_cases
    );
}
