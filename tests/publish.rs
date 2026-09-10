//! End-to-end checks for `regent publish` that stay offline: every case either
//! stops at a dry run or fails before any HTTP request is made.

use std::fs;
use std::path::{Path, PathBuf};

use regent::publisher::{
    Credentials, HttpMethod, PublishConfig, PublishTarget, Publisher, DEFAULT_FORGE_URL,
};

fn write_module(dir: &Path, extra_metadata: &str) {
    fs::create_dir_all(dir.join("manifests")).unwrap();
    fs::write(
        dir.join("metadata.json"),
        format!(
            r#"{{
  "name": "acme-web",
  "version": "1.2.3",
  "author": "acme",
  "license": "Apache-2.0",
  "summary": "Manages web servers",
  "source": "https://github.com/acme/puppet-web",
  "project_page": "https://github.com/acme/puppet-web",
  "dependencies": []{extra_metadata}
}}"#
        ),
    )
    .unwrap();
    fs::write(dir.join("manifests").join("init.pp"), "class web {}\n").unwrap();
}

/// A stand-in for a built package, so the publisher doesn't have to build one.
fn write_tarball(dir: &Path) -> PathBuf {
    let pkg = dir.join("pkg");
    fs::create_dir_all(&pkg).unwrap();
    let tarball = pkg.join("acme-web-1.2.3.tar.gz");
    fs::write(&tarball, b"\x1f\x8bnot-really-a-tarball").unwrap();
    tarball
}

fn generic_target() -> PublishTarget {
    PublishTarget::Generic {
        url: "https://repo.example.com/modules/{name}/{version}/{filename}".to_string(),
        method: HttpMethod::Put,
    }
}

#[test]
fn dry_run_reports_the_expanded_destination_url() {
    let tmp = tempfile::tempdir().unwrap();
    write_module(tmp.path(), "");
    write_tarball(tmp.path());

    let outcome = Publisher::new(
        PublishConfig::new(tmp.path())
            .with_target(generic_target())
            .dry_run(true),
    )
    .publish()
    .unwrap();

    assert!(outcome.dry_run);
    assert_eq!(outcome.module_name, "acme-web");
    assert_eq!(outcome.version, "1.2.3");
    assert_eq!(
        outcome.url,
        "https://repo.example.com/modules/acme-web/1.2.3/acme-web-1.2.3.tar.gz"
    );
    assert!(outcome.tarball.ends_with("acme-web-1.2.3.tar.gz"));
    assert!(outcome.response_body.is_none());
}

#[test]
fn dry_run_picks_up_the_package_in_pkg() {
    let tmp = tempfile::tempdir().unwrap();
    write_module(tmp.path(), "");
    let expected = write_tarball(tmp.path());

    let outcome = Publisher::new(
        PublishConfig::new(tmp.path())
            .with_target(generic_target())
            .dry_run(true),
    )
    .publish()
    .unwrap();

    assert_eq!(
        outcome.tarball.canonicalize().unwrap(),
        expected.canonicalize().unwrap()
    );
}

#[test]
fn an_explicit_tarball_is_used_verbatim() {
    let tmp = tempfile::tempdir().unwrap();
    write_module(tmp.path(), "");
    write_tarball(tmp.path());
    let other = tmp.path().join("acme-web-9.9.9.tar.gz");
    fs::write(&other, b"\x1f\x8bother").unwrap();

    let outcome = Publisher::new(
        PublishConfig::new(tmp.path())
            .with_target(generic_target())
            .with_tarball(&other)
            .dry_run(true),
    )
    .publish()
    .unwrap();

    assert_eq!(
        outcome.tarball.canonicalize().unwrap(),
        other.canonicalize().unwrap()
    );
}

#[test]
fn a_missing_explicit_tarball_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    write_module(tmp.path(), "");

    let err = Publisher::new(
        PublishConfig::new(tmp.path())
            .with_target(generic_target())
            .with_tarball("pkg/nope.tar.gz")
            .dry_run(true),
    )
    .publish()
    .unwrap_err();

    assert!(err.to_string().contains("Tarball not found"), "{err}");
}

#[test]
fn no_build_refuses_to_build_a_missing_package() {
    let tmp = tempfile::tempdir().unwrap();
    write_module(tmp.path(), "");

    let mut config = PublishConfig::new(tmp.path())
        .with_target(generic_target())
        .dry_run(true);
    config.build_if_missing = false;

    let err = Publisher::new(config).publish().unwrap_err();
    let message = err.to_string();
    assert!(message.contains("acme-web-1.2.3.tar.gz"), "{message}");
    assert!(message.contains("regent build"), "{message}");
}

#[test]
fn publishing_a_directory_without_metadata_fails() {
    let tmp = tempfile::tempdir().unwrap();

    let err = Publisher::new(
        PublishConfig::new(tmp.path())
            .with_target(generic_target())
            .dry_run(true),
    )
    .publish()
    .unwrap_err();

    assert!(err.to_string().contains("No module found"), "{err}");
}

#[test]
fn uploading_without_credentials_fails_before_any_request() {
    let tmp = tempfile::tempdir().unwrap();
    write_module(tmp.path(), "");
    write_tarball(tmp.path());

    // dry_run = false, but Credentials::None short-circuits ahead of the HTTP
    // call, so this stays offline.
    let err = Publisher::new(
        PublishConfig::new(tmp.path())
            .with_target(generic_target())
            .with_credentials(Credentials::None),
    )
    .publish()
    .unwrap_err();

    let message = err.to_string();
    assert!(message.contains("No credentials"), "{message}");
    assert!(message.contains("REGENT_FORGE_TOKEN"), "{message}");
}

#[test]
fn the_default_target_is_the_public_forge_releases_endpoint() {
    let config = PublishConfig::new(".");
    assert_eq!(
        config.target,
        PublishTarget::Forge {
            base_url: DEFAULT_FORGE_URL.to_string()
        }
    );
    assert!(config.target.describe().contains("forgeapi.puppet.com"));
}

#[test]
fn invalid_metadata_is_rejected_before_upload() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
        tmp.path().join("metadata.json"),
        r#"{"name":"acme-web","version":"not-semver","author":"acme","license":"Apache-2.0"}"#,
    )
    .unwrap();

    let err = Publisher::new(
        PublishConfig::new(tmp.path())
            .with_target(generic_target())
            .dry_run(true),
    )
    .publish()
    .unwrap_err();

    assert!(err.to_string().contains("metadata.json"), "{err}");
}
