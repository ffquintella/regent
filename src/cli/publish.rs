use colored::*;
use std::path::Path;

use regent::publisher::{
    parse_header, resolve_token, Credentials, HttpMethod, PublishConfig, PublishTarget, Publisher,
    DEFAULT_FORGE_URL,
};

/// Options collected from the `regent publish` flags.
pub struct PublishOptions<'a> {
    pub path: &'a Path,
    pub file: Option<&'a Path>,
    pub no_build: bool,
    pub forge_url: &'a str,
    pub url: Option<&'a str>,
    pub method: &'a str,
    pub token: Option<&'a str>,
    pub token_file: Option<&'a Path>,
    pub username: Option<&'a str>,
    pub password: Option<&'a str>,
    pub headers: &'a [String],
    pub force: bool,
    pub dry_run: bool,
}

pub struct PublishCommand;

impl PublishCommand {
    pub fn execute(opts: PublishOptions<'_>) -> anyhow::Result<()> {
        let target = match opts.url {
            Some(url) => PublishTarget::Generic {
                url: url.to_string(),
                method: HttpMethod::parse(opts.method)?,
            },
            None => PublishTarget::Forge {
                base_url: opts.forge_url.to_string(),
            },
        };

        let credentials = resolve_credentials(&opts)?;

        let headers = opts
            .headers
            .iter()
            .map(|raw| parse_header(raw))
            .collect::<anyhow::Result<Vec<_>>>()?;

        let mut config = PublishConfig::new(opts.path)
            .with_target(target)
            .with_credentials(credentials)
            .dry_run(opts.dry_run);
        config.build_if_missing = !opts.no_build;
        config.force = opts.force;
        config.headers = headers;
        if let Some(file) = opts.file {
            config = config.with_tarball(file);
        }

        println!("{} Publishing to {}", "⚙".cyan(), config.target.describe());

        let outcome = Publisher::new(config).publish()?;

        if outcome.dry_run {
            println!(
                "{} Dry run: would upload {} ({}-{}) to {}",
                "→".yellow().bold(),
                outcome.tarball.display(),
                outcome.module_name,
                outcome.version,
                outcome.url
            );
            return Ok(());
        }

        println!(
            "{} Published {} {} → {}",
            "✓".green().bold(),
            outcome.module_name,
            outcome.version,
            outcome.url
        );
        if let Some(body) = outcome.response_body {
            log::debug!("repository response: {body}");
        }

        Ok(())
    }
}

/// Bearer token by default; Basic auth when a username is supplied.
fn resolve_credentials(opts: &PublishOptions<'_>) -> anyhow::Result<Credentials> {
    if let Some(username) = opts.username {
        let password = match opts.password {
            Some(password) => password.to_string(),
            None => std::env::var("REGENT_PUBLISH_PASSWORD").map_err(|_| {
                anyhow::anyhow!(
                    "--username given without a password; pass --password or set \
                     REGENT_PUBLISH_PASSWORD"
                )
            })?,
        };
        return Ok(Credentials::Basic {
            username: username.to_string(),
            password,
        });
    }

    match resolve_token(opts.token, opts.token_file)? {
        Some(token) => Ok(Credentials::Bearer(token)),
        None => Ok(Credentials::None),
    }
}

/// Default Forge base URL, exposed so `main.rs` can use it as a clap default.
pub const DEFAULT_FORGE_BASE_URL: &str = DEFAULT_FORGE_URL;
