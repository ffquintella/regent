//! Exercise packaged bootstrap without the invoking user's cache or tools.
#![cfg(unix)]

use regent::tester::bundled_gems::verify_required_gems;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

fn bootstrap(binary: &Path, home: &Path, cwd: &Path) {
    let output = Command::new(binary)
        .arg("bootstrap")
        .env_clear()
        .env("HOME", home)
        .env("PATH", "")
        .env("REGENT_BUNDLED_GEMS", cwd.join("incomplete-cache"))
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "bootstrap failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn fresh_home_bootstraps_from_packaged_sidecar_without_host_ruby() {
    let stage = tempdir().unwrap();
    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap();
    // An inherited override with only the old layout must not hide the
    // complete packaged or embedded payload.
    fs::create_dir_all(cwd.path().join("incomplete-cache/ruby/2.6.0/gems")).unwrap();
    let binary = stage.path().join("regent");
    fs::copy(env!("CARGO_BIN_EXE_regent"), &binary).unwrap();
    let sidecar = stage.path().join("bundled_gems");
    let archived = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/bundled_gems/cache.tar.gz"
    ));
    tar::Archive::new(flate2::read::GzDecoder::new(&archived[..]))
        .unpack(&sidecar)
        .unwrap();
    verify_required_gems(&sidecar).unwrap();
    // This marker proves the exe-relative packaged cache was selected instead
    // of the build-time manifest's development fallback.
    fs::write(sidecar.join("packaged-source-marker"), b"sidecar").unwrap();
    let installed = home.path().join(".regent/bundle");
    assert!(!installed.exists());
    bootstrap(&binary, home.path(), cwd.path());
    verify_required_gems(&installed).unwrap();
    assert_eq!(
        fs::read(installed.join("packaged-source-marker")).unwrap(),
        b"sidecar"
    );

    // Repeat after the installer payload has been removed. The original cache
    // should not be consulted once the per-user bundle is complete.
    fs::remove_dir_all(sidecar).unwrap();
    bootstrap(&binary, home.path(), cwd.path());
    verify_required_gems(&installed).unwrap();
    assert!(installed.join("packaged-source-marker").is_file());

    // A fresh home and no sidecar must use the embedded archive, independent
    // of the original build checkout and any host Ruby tools.
    let embedded_home = tempdir().unwrap();
    bootstrap(&binary, embedded_home.path(), cwd.path());
    let embedded_install = embedded_home.path().join(".regent/bundle");
    verify_required_gems(&embedded_install).unwrap();
    assert!(!embedded_install.join("packaged-source-marker").exists());
}

#[test]
fn cli_rspec_requires_preserve_embedded_assertion_verdicts() {
    let home = tempdir().unwrap();
    let module = tempdir().unwrap();
    fs::create_dir_all(module.path().join("spec/classes")).unwrap();
    fs::create_dir_all(module.path().join("manifests")).unwrap();
    let spec = module.path().join("spec/classes/required_rspec_spec.rb");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_regent"))
            .arg("test")
            .arg(module.path())
            .arg("--detail")
            .env_clear()
            .env("HOME", home.path())
            .env("PATH", "")
            .current_dir(module.path())
            .output()
            .unwrap()
    };
    let requires = "require 'rspec'\nrequire 'rspec/core'\nrequire 'rspec/expectations'\n";
    fs::write(
        &spec,
        format!("{requires}describe 'embedded assertions' do\n  it('passes') {{ expect(2 + 2).to eq(4) }}\nend\n"),
    )
    .unwrap();
    let passing = run();
    let stdout = String::from_utf8_lossy(&passing.stdout);
    assert!(
        passing.status.success(),
        "stdout={stdout} stderr={}",
        String::from_utf8_lossy(&passing.stderr)
    );
    assert!(stdout.contains("Passed: 1, Failed: 0, Skipped: 0"));

    fs::write(
        &spec,
        format!("{requires}describe 'embedded assertions' do\n  it('fails') {{ expect(2 + 2).to eq(5) }}\nend\n"),
    )
    .unwrap();
    let failing = run();
    let stdout = String::from_utf8_lossy(&failing.stdout);
    assert!(!failing.status.success(), "stdout={stdout}");
    assert!(stdout.contains("Passed: 0, Failed: 1, Skipped: 0"));
    assert!(stdout.contains("expected 4 to eq 5"), "stdout={stdout}");
    assert!(!String::from_utf8_lossy(&failing.stderr).contains("Run `regent bootstrap`"));
}
