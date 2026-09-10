//! Module publishing: upload a built module tarball to the Puppet Forge or to
//! any other HTTP-reachable module repository.
//!
//! Publishing never shells out to `puppet`, `gem` or a host Ruby: the tarball is
//! produced by [`crate::builder`] and uploaded over HTTP from Rust.

pub mod upload;

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

use crate::builder::{ModuleBuilder, ModuleMetadata};

/// Default Forge API base URL.
pub const DEFAULT_FORGE_URL: &str = "https://forgeapi.puppet.com";

/// Environment variables consulted for a publish token, in order.
const TOKEN_ENV_VARS: [&str; 3] = [
    "REGENT_FORGE_TOKEN",
    "REGENT_PUBLISH_TOKEN",
    // Accepted for convenience so existing PDK setups keep working.
    "PDK_FORGE_TOKEN",
];

/// HTTP method used for generic (non-Forge) repositories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Put,
    Post,
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Put => "PUT",
            HttpMethod::Post => "POST",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "put" => Ok(HttpMethod::Put),
            "post" => Ok(HttpMethod::Post),
            other => Err(anyhow!(
                "unsupported HTTP method `{}` (expected put or post)",
                other
            )),
        }
    }
}

/// Where a module should be published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishTarget {
    /// The Puppet Forge, or any Forge-API-v3-compatible mirror.
    ///
    /// The release is uploaded as `multipart/form-data` to
    /// `<base_url>/v3/releases`, which is what the Forge release endpoint
    /// expects.
    Forge { base_url: String },

    /// An arbitrary HTTP endpoint that accepts the raw tarball as the request
    /// body (Artifactory generic repos, Nexus raw repos, an S3 pre-signed URL,
    /// an internal module mirror, …).
    Generic { url: String, method: HttpMethod },
}

impl PublishTarget {
    /// Human-readable description used in log lines.
    pub fn describe(&self) -> String {
        match self {
            PublishTarget::Forge { base_url } => format!("Puppet Forge at {base_url}"),
            PublishTarget::Generic { url, method } => format!("{} {}", method.as_str(), url),
        }
    }
}

/// Credentials presented to the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Credentials {
    None,
    Bearer(String),
    Basic { username: String, password: String },
}

impl Credentials {
    /// Value for the `Authorization` header, if any.
    pub fn authorization_header(&self) -> Option<String> {
        match self {
            Credentials::None => None,
            Credentials::Bearer(token) => Some(format!("Bearer {token}")),
            Credentials::Basic { username, password } => Some(format!(
                "Basic {}",
                base64_encode(format!("{username}:{password}").as_bytes())
            )),
        }
    }
}

/// Everything needed for one publish run.
#[derive(Debug, Clone)]
pub struct PublishConfig {
    /// Path to the module (the directory holding `metadata.json`).
    pub module_path: PathBuf,
    /// Explicit tarball to publish; when `None` the tarball is looked up in
    /// `<module_path>/pkg` and built if missing.
    pub tarball: Option<PathBuf>,
    /// Build the tarball when it isn't already present.
    pub build_if_missing: bool,
    pub target: PublishTarget,
    pub credentials: Credentials,
    /// Extra request headers as `(name, value)` pairs.
    pub headers: Vec<(String, String)>,
    /// Publish even when the Forge already has this version.
    pub force: bool,
    /// Do everything except the upload itself.
    pub dry_run: bool,
}

impl PublishConfig {
    pub fn new(module_path: impl Into<PathBuf>) -> Self {
        Self {
            module_path: module_path.into(),
            tarball: None,
            build_if_missing: true,
            target: PublishTarget::Forge {
                base_url: DEFAULT_FORGE_URL.to_string(),
            },
            credentials: Credentials::None,
            headers: Vec::new(),
            force: false,
            dry_run: false,
        }
    }

    pub fn with_target(mut self, target: PublishTarget) -> Self {
        self.target = target;
        self
    }

    pub fn with_credentials(mut self, credentials: Credentials) -> Self {
        self.credentials = credentials;
        self
    }

    pub fn with_tarball(mut self, tarball: impl Into<PathBuf>) -> Self {
        self.tarball = Some(tarball.into());
        self
    }

    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }
}

/// Result of a publish run.
#[derive(Debug, Clone)]
pub struct PublishOutcome {
    pub module_name: String,
    pub version: String,
    /// Tarball that was (or would have been) uploaded.
    pub tarball: PathBuf,
    /// Fully resolved destination URL.
    pub url: String,
    /// True when nothing was actually uploaded.
    pub dry_run: bool,
    /// Response body returned by the repository, when it sent one.
    pub response_body: Option<String>,
}

pub struct Publisher {
    config: PublishConfig,
}

impl Publisher {
    pub fn new(config: PublishConfig) -> Self {
        Self { config }
    }

    /// Validate, locate (or build) the tarball, and upload it.
    pub fn publish(&self) -> Result<PublishOutcome> {
        let module_path = self
            .config
            .module_path
            .canonicalize()
            .unwrap_or_else(|_| self.config.module_path.clone());

        let metadata_path = module_path.join("metadata.json");
        if !metadata_path.exists() {
            return Err(anyhow!("No module found at {}", module_path.display()));
        }
        let metadata = ModuleMetadata::load(&metadata_path)
            .with_context(|| format!("Failed to load {}", metadata_path.display()))?;

        for problem in publish_readiness_warnings(&metadata) {
            log::warn!("{problem}");
        }

        let slug = metadata.name.replace("::", "-");
        let filename = format!("{}-{}.tar.gz", slug, metadata.version);

        let tarball = self.resolve_tarball(&module_path, &filename)?;

        if let PublishTarget::Forge { base_url } = &self.config.target {
            if !self.config.force
                && upload::forge_release_exists(base_url, &slug, &metadata.version)?
            {
                return Err(anyhow!(
                    "{}-{} is already published on {} (use --force to upload anyway, \
                     or bump the version in metadata.json)",
                    slug,
                    metadata.version,
                    base_url
                ));
            }
        }

        let url = self.resolve_url(&slug, &metadata.version, &filename);

        if self.config.dry_run {
            return Ok(PublishOutcome {
                module_name: metadata.name,
                version: metadata.version,
                tarball,
                url,
                dry_run: true,
                response_body: None,
            });
        }

        if matches!(self.config.credentials, Credentials::None) {
            return Err(anyhow!(
                "No credentials for {}.\n  Pass --token/--token-file, or set one of: {}",
                self.config.target.describe(),
                TOKEN_ENV_VARS.join(", ")
            ));
        }

        let response_body = match &self.config.target {
            PublishTarget::Forge { .. } => upload::upload_multipart(
                &url,
                &tarball,
                &filename,
                &self.config.credentials,
                &self.config.headers,
            )?,
            PublishTarget::Generic { method, .. } => upload::upload_raw(
                &url,
                *method,
                &tarball,
                &self.config.credentials,
                &self.config.headers,
            )?,
        };

        Ok(PublishOutcome {
            module_name: metadata.name,
            version: metadata.version,
            tarball,
            url,
            dry_run: false,
            response_body,
        })
    }

    /// Find the tarball to publish, building it when allowed.
    fn resolve_tarball(&self, module_path: &Path, filename: &str) -> Result<PathBuf> {
        if let Some(explicit) = &self.config.tarball {
            let path = if explicit.is_absolute() {
                explicit.clone()
            } else {
                module_path.join(explicit)
            };
            if !path.exists() {
                return Err(anyhow!("Tarball not found: {}", path.display()));
            }
            return Ok(path);
        }

        let expected = module_path.join("pkg").join(filename);
        if expected.exists() {
            return Ok(expected);
        }

        if !self.config.build_if_missing {
            return Err(anyhow!(
                "{} does not exist. Run `regent build` first, or drop --no-build.",
                expected.display()
            ));
        }

        log::info!("{} not found; building it", expected.display());
        let artifact = ModuleBuilder::build(module_path, None, None)
            .context("Failed to build module before publishing")?;
        Ok(artifact.tarball_path)
    }

    /// Destination URL, with `{name}`, `{version}` and `{filename}` expanded
    /// for generic targets.
    fn resolve_url(&self, slug: &str, version: &str, filename: &str) -> String {
        match &self.config.target {
            PublishTarget::Forge { base_url } => {
                format!("{}/v3/releases", base_url.trim_end_matches('/'))
            }
            PublishTarget::Generic { url, .. } => expand_url(url, slug, version, filename),
        }
    }
}

/// Expand the placeholders supported in a generic repository URL.
pub fn expand_url(url: &str, slug: &str, version: &str, filename: &str) -> String {
    url.replace("{name}", slug)
        .replace("{version}", version)
        .replace("{filename}", filename)
}

/// Non-fatal metadata gaps worth telling the user about before an upload.
///
/// These are the fields the Forge shows on a module page; a release without
/// them is accepted but looks unfinished.
pub fn publish_readiness_warnings(metadata: &ModuleMetadata) -> Vec<String> {
    let mut warnings = Vec::new();
    if metadata.summary.trim().is_empty() {
        warnings.push("metadata.json has no `summary`".to_string());
    }
    if metadata.source.trim().is_empty() {
        warnings.push("metadata.json has no `source`".to_string());
    }
    if metadata.project_page.trim().is_empty() {
        warnings.push("metadata.json has no `project_page`".to_string());
    }
    if metadata.dependencies.is_empty() {
        log::debug!("module declares no dependencies");
    }
    warnings
}

/// Resolve a publish token from the explicit flag, a token file, the
/// environment, or `~/.regent/forge_token`.
pub fn resolve_token(explicit: Option<&str>, token_file: Option<&Path>) -> Result<Option<String>> {
    if let Some(token) = explicit {
        let token = token.trim();
        if !token.is_empty() {
            return Ok(Some(token.to_string()));
        }
    }

    if let Some(path) = token_file {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read token file {}", path.display()))?;
        let token = contents.trim();
        if token.is_empty() {
            return Err(anyhow!("Token file {} is empty", path.display()));
        }
        return Ok(Some(token.to_string()));
    }

    for var in TOKEN_ENV_VARS {
        if let Ok(value) = std::env::var(var) {
            let value = value.trim();
            if !value.is_empty() {
                log::debug!("using publish token from ${var}");
                return Ok(Some(value.to_string()));
            }
        }
    }

    if let Some(default_file) = default_token_file() {
        if default_file.exists() {
            let contents = std::fs::read_to_string(&default_file)
                .with_context(|| format!("Failed to read {}", default_file.display()))?;
            let token = contents.trim();
            if !token.is_empty() {
                log::debug!("using publish token from {}", default_file.display());
                return Ok(Some(token.to_string()));
            }
        }
    }

    Ok(None)
}

/// `~/.regent/forge_token` (Unix) / `%APPDATA%\Regent\forge_token` (Windows).
pub fn default_token_file() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return Some(PathBuf::from(appdata).join("Regent").join("forge_token"));
        }
        std::env::var("USERPROFILE")
            .ok()
            .map(|home| PathBuf::from(home).join(".regent").join("forge_token"))
    }
    #[cfg(not(windows))]
    {
        std::env::var("HOME")
            .ok()
            .map(|home| PathBuf::from(home).join(".regent").join("forge_token"))
    }
}

/// Parse a `Name: value` or `Name=value` header argument.
pub fn parse_header(raw: &str) -> Result<(String, String)> {
    let (name, value) = raw
        .split_once(':')
        .or_else(|| raw.split_once('='))
        .ok_or_else(|| anyhow!("invalid header `{}` (expected `Name: value`)", raw))?;
    let name = name.trim();
    let value = value.trim();
    if name.is_empty() {
        return Err(anyhow!("invalid header `{}` (empty header name)", raw));
    }
    Ok((name.to_string(), value.to_string()))
}

/// Minimal RFC 4648 base64 encoder, used for HTTP Basic credentials.
fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(triple >> 18) as usize & 0x3f] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 0x3f] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(triple >> 6) as usize & 0x3f] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[triple as usize & 0x3f] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"user:pass"), "dXNlcjpwYXNz");
    }

    #[test]
    fn basic_auth_header_is_encoded() {
        let creds = Credentials::Basic {
            username: "user".to_string(),
            password: "pass".to_string(),
        };
        assert_eq!(
            creds.authorization_header().as_deref(),
            Some("Basic dXNlcjpwYXNz")
        );
    }

    #[test]
    fn bearer_header_uses_token() {
        let creds = Credentials::Bearer("abc123".to_string());
        assert_eq!(
            creds.authorization_header().as_deref(),
            Some("Bearer abc123")
        );
        assert!(Credentials::None.authorization_header().is_none());
    }

    #[test]
    fn forge_url_is_the_releases_endpoint() {
        let publisher = Publisher::new(PublishConfig::new(".").with_target(PublishTarget::Forge {
            base_url: "https://forgeapi.puppet.com/".to_string(),
        }));
        assert_eq!(
            publisher.resolve_url(
                "puppetlabs-stdlib",
                "9.0.0",
                "puppetlabs-stdlib-9.0.0.tar.gz"
            ),
            "https://forgeapi.puppet.com/v3/releases"
        );
    }

    #[test]
    fn generic_url_placeholders_are_expanded() {
        assert_eq!(
            expand_url(
                "https://repo.example.com/modules/{name}/{version}/{filename}",
                "acme-web",
                "1.2.3",
                "acme-web-1.2.3.tar.gz"
            ),
            "https://repo.example.com/modules/acme-web/1.2.3/acme-web-1.2.3.tar.gz"
        );
    }

    #[test]
    fn generic_url_without_placeholders_is_untouched() {
        assert_eq!(
            expand_url(
                "https://repo.example.com/upload",
                "a-b",
                "1.0.0",
                "x.tar.gz"
            ),
            "https://repo.example.com/upload"
        );
    }

    #[test]
    fn headers_accept_colon_and_equals() {
        assert_eq!(
            parse_header("X-Api-Key: secret").unwrap(),
            ("X-Api-Key".to_string(), "secret".to_string())
        );
        assert_eq!(
            parse_header("X-Api-Key=secret").unwrap(),
            ("X-Api-Key".to_string(), "secret".to_string())
        );
        assert!(parse_header("nonsense").is_err());
        assert!(parse_header(": value").is_err());
    }

    #[test]
    fn http_method_parsing_is_case_insensitive() {
        assert_eq!(HttpMethod::parse("PUT").unwrap(), HttpMethod::Put);
        assert_eq!(HttpMethod::parse("post").unwrap(), HttpMethod::Post);
        assert!(HttpMethod::parse("delete").is_err());
    }

    #[test]
    fn explicit_token_wins_over_token_file() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        use std::io::Write;
        writeln!(file, "from-file").unwrap();
        let token = resolve_token(Some("explicit"), Some(file.path())).unwrap();
        assert_eq!(token.as_deref(), Some("explicit"));
    }

    #[test]
    fn token_file_contents_are_trimmed() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        use std::io::Write;
        writeln!(file, "  from-file  ").unwrap();
        let token = resolve_token(None, Some(file.path())).unwrap();
        assert_eq!(token.as_deref(), Some("from-file"));
    }

    #[test]
    fn empty_token_file_is_an_error() {
        let file = tempfile::NamedTempFile::new().unwrap();
        assert!(resolve_token(None, Some(file.path())).is_err());
    }

    #[test]
    fn readiness_warnings_flag_missing_forge_fields() {
        let metadata = ModuleMetadata {
            name: "acme-web".to_string(),
            version: "1.0.0".to_string(),
            author: "acme".to_string(),
            license: "Apache-2.0".to_string(),
            summary: String::new(),
            description: String::new(),
            project_page: "https://example.com".to_string(),
            source: String::new(),
            issues_url: String::new(),
            dependencies: Vec::new(),
            requirements: Vec::new(),
            operatingsystem_support: Vec::new(),
            tags: Vec::new(),
            template_version: String::new(),
        };
        let warnings = publish_readiness_warnings(&metadata);
        assert!(warnings.iter().any(|w| w.contains("summary")));
        assert!(warnings.iter().any(|w| w.contains("source")));
        assert!(!warnings.iter().any(|w| w.contains("project_page")));
    }
}
