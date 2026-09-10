//! HTTP transport for `regent publish`.

use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::time::Duration;

use super::{Credentials, HttpMethod};

/// Upload timeout; module tarballs are small but forge round-trips can be slow.
const UPLOAD_TIMEOUT: Duration = Duration::from_secs(300);
const QUERY_TIMEOUT: Duration = Duration::from_secs(30);

/// Ask a Forge API whether `<slug>-<version>` has already been released.
///
/// A network or server error is *not* treated as "already published" — the
/// check is a courtesy, so it fails open and lets the upload decide.
pub fn forge_release_exists(base_url: &str, slug: &str, version: &str) -> Result<bool> {
    let url = format!(
        "{}/v3/releases/{}-{}",
        base_url.trim_end_matches('/'),
        slug,
        version
    );
    match ureq::get(&url).timeout(QUERY_TIMEOUT).call() {
        Ok(_) => Ok(true),
        Err(ureq::Error::Status(404, _)) => Ok(false),
        Err(ureq::Error::Status(code, _)) => {
            log::debug!("release lookup {url} returned HTTP {code}; continuing");
            Ok(false)
        }
        Err(err) => {
            log::debug!("release lookup {url} failed: {err}; continuing");
            Ok(false)
        }
    }
}

/// POST the tarball as `multipart/form-data` with a single `file` part, which
/// is what the Forge `/v3/releases` endpoint expects.
pub fn upload_multipart(
    url: &str,
    tarball: &Path,
    filename: &str,
    credentials: &Credentials,
    headers: &[(String, String)],
) -> Result<Option<String>> {
    let bytes =
        std::fs::read(tarball).with_context(|| format!("Failed to read {}", tarball.display()))?;
    let boundary = multipart_boundary();
    let body = build_multipart_body(&boundary, filename, &bytes)?;

    let mut request = ureq::post(url)
        .timeout(UPLOAD_TIMEOUT)
        .set(
            "Content-Type",
            &format!("multipart/form-data; boundary={boundary}"),
        )
        .set("Accept", "application/json");
    if let Some(auth) = credentials.authorization_header() {
        request = request.set("Authorization", &auth);
    }
    for (name, value) in headers {
        request = request.set(name, value);
    }

    send(request.send_bytes(&body), url)
}

/// Send the tarball as the raw request body (generic repositories).
pub fn upload_raw(
    url: &str,
    method: HttpMethod,
    tarball: &Path,
    credentials: &Credentials,
    headers: &[(String, String)],
) -> Result<Option<String>> {
    let bytes =
        std::fs::read(tarball).with_context(|| format!("Failed to read {}", tarball.display()))?;

    let mut request = match method {
        HttpMethod::Put => ureq::put(url),
        HttpMethod::Post => ureq::post(url),
    }
    .timeout(UPLOAD_TIMEOUT)
    .set("Content-Type", "application/gzip");
    if let Some(auth) = credentials.authorization_header() {
        request = request.set("Authorization", &auth);
    }
    for (name, value) in headers {
        request = request.set(name, value);
    }

    send(request.send_bytes(&bytes), url)
}

/// Turn a ureq result into a body string, surfacing the server's own error
/// message (which is where Forge validation failures live).
fn send(result: Result<ureq::Response, ureq::Error>, url: &str) -> Result<Option<String>> {
    match result {
        Ok(response) => {
            let body = response.into_string().unwrap_or_default();
            let body = body.trim().to_string();
            Ok(if body.is_empty() { None } else { Some(body) })
        }
        Err(ureq::Error::Status(code, response)) => {
            let body = response.into_string().unwrap_or_default();
            Err(anyhow!(
                "{} rejected the upload: HTTP {}{}",
                url,
                code,
                describe_error_body(&body)
            ))
        }
        Err(err) => Err(anyhow!("Failed to upload to {}: {}", url, err)),
    }
}

/// Pull a human message out of an error body, preferring the JSON `message` /
/// `errors` fields that Forge-style APIs return.
fn describe_error_body(body: &str) -> String {
    let body = body.trim();
    if body.is_empty() {
        return String::new();
    }
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(message) = json.get("message").and_then(|v| v.as_str()) {
            return format!("\n  {message}");
        }
        if let Some(errors) = json.get("errors").and_then(|v| v.as_array()) {
            let joined: Vec<String> = errors
                .iter()
                .map(|e| e.as_str().unwrap_or("").trim().to_string())
                .filter(|e| !e.is_empty())
                .collect();
            if !joined.is_empty() {
                return format!("\n  {}", joined.join("\n  "));
            }
        }
    }
    format!("\n  {body}")
}

/// A boundary token that is valid per RFC 2046 and unlikely to collide.
fn multipart_boundary() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("RegentBoundary{nanos:x}{:x}", std::process::id())
}

/// Assemble the multipart body for a single `file` part.
///
/// Errors if the payload happens to contain the boundary, rather than sending a
/// body the server would parse as truncated.
fn build_multipart_body(boundary: &str, filename: &str, bytes: &[u8]) -> Result<Vec<u8>> {
    if contains(bytes, boundary.as_bytes()) {
        return Err(anyhow!(
            "tarball contains the generated multipart boundary; retry the publish"
        ));
    }

    let header = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
         Content-Type: application/gzip\r\n\r\n"
    );
    let footer = format!("\r\n--{boundary}--\r\n");

    let mut body = Vec::with_capacity(header.len() + bytes.len() + footer.len());
    body.extend_from_slice(header.as_bytes());
    body.extend_from_slice(bytes);
    body.extend_from_slice(footer.as_bytes());
    Ok(body)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multipart_body_wraps_the_payload() {
        let body =
            build_multipart_body("BOUND", "acme-web-1.0.0.tar.gz", b"\x1f\x8btarball").unwrap();
        let text = String::from_utf8_lossy(&body);
        assert!(text.starts_with("--BOUND\r\nContent-Disposition: form-data; name=\"file\"; filename=\"acme-web-1.0.0.tar.gz\"\r\n"));
        assert!(text.contains("Content-Type: application/gzip\r\n\r\n"));
        assert!(text.ends_with("\r\n--BOUND--\r\n"));
        assert!(contains(&body, b"\x1f\x8btarball"));
    }

    #[test]
    fn multipart_body_rejects_a_colliding_boundary() {
        let err = build_multipart_body("BOUND", "x.tar.gz", b"payload BOUND payload").unwrap_err();
        assert!(err.to_string().contains("multipart boundary"));
    }

    #[test]
    fn boundary_is_token_safe() {
        let boundary = multipart_boundary();
        assert!(boundary.len() <= 70, "boundary too long: {boundary}");
        assert!(boundary.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn error_bodies_prefer_the_json_message() {
        assert_eq!(
            describe_error_body(r#"{"message":"Version already exists"}"#),
            "\n  Version already exists"
        );
        assert_eq!(
            describe_error_body(r#"{"errors":["bad metadata","no license"]}"#),
            "\n  bad metadata\n  no license"
        );
        assert_eq!(describe_error_body("plain text"), "\n  plain text");
        assert_eq!(describe_error_body("   "), "");
    }

    #[test]
    fn substring_search_handles_edges() {
        assert!(contains(b"abcdef", b"cde"));
        assert!(!contains(b"abc", b"abcd"));
        assert!(!contains(b"abc", b""));
    }
}
