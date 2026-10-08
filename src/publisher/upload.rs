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
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(QUERY_TIMEOUT))
        .build()
        .new_agent();
    match agent.get(&url).call() {
        Ok(_) => Ok(true),
        Err(ureq::Error::StatusCode(404)) => Ok(false),
        Err(ureq::Error::StatusCode(code)) => {
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

    let agent = upload_agent();
    let mut request = agent
        .post(url)
        .header(
            "Content-Type",
            &format!("multipart/form-data; boundary={boundary}"),
        )
        .header("Accept", "application/json");
    if let Some(auth) = credentials.authorization_header() {
        request = request.header("Authorization", &auth);
    }
    for (name, value) in headers {
        // ureq 2's set() replaced earlier values; ureq 3's header() appends.
        if let Some(request_headers) = request.headers_mut() {
            request_headers.remove(name.as_str());
        }
        request = request.header(name.as_str(), value.as_str());
    }

    send(request.send(&body), url)
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

    let agent = upload_agent();
    let mut request = match method {
        HttpMethod::Put => agent.put(url),
        HttpMethod::Post => agent.post(url),
    }
    .header("Content-Type", "application/gzip");
    if let Some(auth) = credentials.authorization_header() {
        request = request.header("Authorization", &auth);
    }
    for (name, value) in headers {
        if let Some(request_headers) = request.headers_mut() {
            request_headers.remove(name.as_str());
        }
        request = request.header(name.as_str(), value.as_str());
    }

    send(request.send(&bytes), url)
}

fn upload_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(UPLOAD_TIMEOUT))
        // ureq 3's StatusCode error drops the response body. Keep the response
        // so Forge's validation message can still be reported to the caller.
        .http_status_as_error(false)
        .build()
        .new_agent()
}

/// Turn a ureq result into a body string, surfacing the server's own error
/// message (which is where Forge validation failures live).
fn send(
    result: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
    url: &str,
) -> Result<Option<String>> {
    match result {
        Ok(mut response) => {
            let status = response.status();
            let body = response.body_mut().read_to_string().unwrap_or_default();
            if status.is_client_error() || status.is_server_error() {
                return Err(anyhow!(
                    "{} rejected the upload: HTTP {}{}",
                    url,
                    status.as_u16(),
                    describe_error_body(&body)
                ));
            }
            let body = body.trim().to_string();
            Ok(if body.is_empty() { None } else { Some(body) })
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
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Instant;

    /// One loopback exchange exercises ureq itself without a Forge account.
    fn serve_once(status: u16, body: &'static str) -> (String, thread::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "no HTTP request received");
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(err) => panic!("accept HTTP request: {err}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(&stream);
            let mut request = Vec::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                request.extend_from_slice(line.as_bytes());
                if line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':') {
                    if name.eq_ignore_ascii_case("content-length") {
                        length = value.trim().parse().unwrap();
                    }
                }
            }
            let mut payload = vec![0; length];
            reader.read_exact(&mut payload).unwrap();
            request.extend_from_slice(&payload);
            write!(
                stream,
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            request
        });
        (url, server)
    }

    #[test]
    fn raw_upload_preserves_payload_headers_and_response() {
        for method in [HttpMethod::Put, HttpMethod::Post] {
            let (url, server) = serve_once(201, "  uploaded  ");
            let tarball = tempfile::NamedTempFile::new().unwrap();
            std::fs::write(tarball.path(), b"\x1f\x8btarball").unwrap();
            let result = upload_raw(
                &url,
                method,
                tarball.path(),
                &Credentials::Bearer("test-token".into()),
                &[
                    ("X-Test".into(), "old-value".into()),
                    ("x-test".into(), "extra-header".into()),
                    ("Content-Type".into(), "application/octet-stream".into()),
                ],
            );
            let request = server.join().unwrap();
            assert_eq!(result.unwrap().as_deref(), Some("uploaded"));
            let headers = String::from_utf8_lossy(&request).to_ascii_lowercase();
            assert!(headers.starts_with(&format!(
                "{} / http/1.1",
                method.as_str().to_ascii_lowercase()
            )));
            assert!(headers.contains("authorization: bearer test-token\r\n"));
            assert!(headers.contains("content-type: application/octet-stream\r\n"));
            assert_eq!(headers.matches("content-type:").count(), 1);
            assert!(headers.contains("x-test: extra-header\r\n"));
            assert_eq!(headers.matches("x-test:").count(), 1);
            assert!(request.ends_with(b"\x1f\x8btarball"));
        }
    }

    #[test]
    fn multipart_upload_keeps_server_validation_errors() {
        for status in [422, 500] {
            let (url, server) = serve_once(status, r#"{"message":"Rejected module"}"#);
            let tarball = tempfile::NamedTempFile::new().unwrap();
            std::fs::write(tarball.path(), b"payload").unwrap();
            let result = upload_multipart(
                &url,
                tarball.path(),
                "acme-web-1.0.0.tar.gz",
                &Credentials::None,
                &[],
            );
            let request = server.join().unwrap();
            let error = result.unwrap_err().to_string();
            assert!(error.contains(&format!("HTTP {status}")), "{error}");
            assert!(error.contains("Rejected module"), "{error}");
            let request = String::from_utf8(request).unwrap();
            assert!(request.starts_with("POST / HTTP/1.1\r\n"));
            assert!(request.contains("multipart/form-data; boundary="));
            assert!(request.contains("filename=\"acme-web-1.0.0.tar.gz\""));
            assert!(request.contains("\r\n\r\npayload\r\n"));
        }
    }

    #[test]
    fn forge_release_lookup_handles_http_statuses() {
        for (status, expected) in [(200, true), (404, false), (503, false)] {
            let (url, server) = serve_once(status, "");
            let result = forge_release_exists(&url, "acme-web", "1.0.0");
            let request = server.join().unwrap();
            assert_eq!(result.unwrap(), expected);
            assert!(request.starts_with(b"GET /v3/releases/acme-web-1.0.0 HTTP/1.1\r\n"));
        }
    }

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
