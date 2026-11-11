//! A minimal HTTP/1.1 GET client over `std::net` for ComfyUI's JSON API
//! (docs/specs/comfyui-connect.md, Decisions: "std HTTP client").
//!
//! Only plain `http://` is spoken: `reqwest` (approved in the Decision Log)
//! panics without a rustls crypto provider, and installing one needs a
//! dependency that is not approved yet (the spec's Open questions).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/// Largest response body read (ComfyUI's `/object_info` with many custom
/// nodes runs to a few MB).
const MAX_BODY: usize = 64 * 1024 * 1024;

/// A server address: scheme, host, port and an optional path prefix (a
/// reverse proxy may serve ComfyUI below `/comfy`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    /// `https://` — parsed, but not spoken yet (see the module docs).
    pub https: bool,
    /// Host name or IP literal, without brackets.
    pub host: String,
    pub port: u16,
    /// Path prefix without a trailing slash; empty for none.
    pub prefix: String,
}

impl Url {
    /// `http://127.0.0.1:<port>`.
    pub fn local(port: u16) -> Self {
        Self {
            https: false,
            host: "127.0.0.1".to_string(),
            port,
            prefix: String::new(),
        }
    }

    /// The URL as text, e.g. `http://127.0.0.1:8188/comfy`.
    pub fn display(&self) -> String {
        let scheme = if self.https { "https" } else { "http" };
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        format!("{scheme}://{host}:{}{}", self.port, self.prefix)
    }
}

/// Parses `http[s]://host[:port][/prefix]`; `None` for anything else.
pub fn parse_url(text: &str) -> Option<Url> {
    let text = text.trim();
    let (https, rest) = match strip_prefix_ignore_case(text, "http://") {
        Some(rest) => (false, rest),
        None => (true, strip_prefix_ignore_case(text, "https://")?),
    };
    // Query and fragment mean nothing for a server address.
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    if authority.is_empty() || authority.contains('@') || authority.contains(char::is_whitespace) {
        return None;
    }
    let (host, port) = if let Some(after) = authority.strip_prefix('[') {
        let end = after.find(']')?;
        let host = &after[..end];
        host.parse::<std::net::Ipv6Addr>().ok()?;
        let tail = &after[end + 1..];
        let port = match tail.strip_prefix(':') {
            Some(p) => Some(p),
            None if tail.is_empty() => None,
            None => return None,
        };
        (host.to_string(), port)
    } else {
        match authority.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), Some(p)),
            None => (authority.to_string(), None),
        }
    };
    if host.is_empty() || host.contains(':') && !authority.starts_with('[') {
        return None;
    }
    let port = match port {
        Some(p) => p.parse::<u16>().ok().filter(|p| *p != 0)?,
        None if https => 443,
        None => 80,
    };
    let prefix = path.trim_end_matches('/').to_string();
    if prefix.contains(char::is_whitespace) {
        return None;
    }
    Some(Url {
        https,
        host,
        port,
        prefix,
    })
}

fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

/// A response: status code and body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Why a request got no HTTP response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    /// No connection, a timeout, or an answer that is not HTTP.
    Unreachable(String),
    /// `https://` (see the module docs).
    Unsupported,
}

/// `GET <prefix><path>` with `timeout` for the whole exchange.
pub fn get(url: &Url, path: &str, timeout: Duration) -> Result<Response, HttpError> {
    if url.https {
        return Err(HttpError::Unsupported);
    }
    let deadline = Instant::now() + timeout;
    let mut stream = connect(url, timeout)?;
    let host = if url.host.contains(':') {
        format!("[{}]:{}", url.host, url.port)
    } else {
        format!("{}:{}", url.host, url.port)
    };
    let request = format!(
        "GET {}{} HTTP/1.1\r\nHost: {host}\r\nAccept: application/json\r\nUser-Agent: Pincel\r\nConnection: close\r\n\r\n",
        url.prefix, path
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| HttpError::Unreachable(e.to_string()))?;
    let mut raw = Vec::new();
    let mut buf = [0u8; 16 * 1024];
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(HttpError::Unreachable("timed out".to_string()));
        }
        stream
            .set_read_timeout(Some(left))
            .map_err(|e| HttpError::Unreachable(e.to_string()))?;
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                raw.extend_from_slice(&buf[..n]);
                if raw.len() > MAX_BODY {
                    return Err(HttpError::Unreachable("response too large".to_string()));
                }
                if complete(&raw) {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(HttpError::Unreachable(e.to_string())),
        }
    }
    parse_response(&raw).ok_or_else(|| HttpError::Unreachable("not an HTTP answer".to_string()))
}

fn connect(url: &Url, timeout: Duration) -> Result<TcpStream, HttpError> {
    let addrs: Vec<SocketAddr> = (url.host.as_str(), url.port)
        .to_socket_addrs()
        .map_err(|e| HttpError::Unreachable(e.to_string()))?
        .collect();
    let mut last = "no address".to_string();
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(stream) => {
                stream
                    .set_write_timeout(Some(timeout))
                    .map_err(|e| HttpError::Unreachable(e.to_string()))?;
                return Ok(stream);
            }
            Err(e) => last = e.to_string(),
        }
    }
    Err(HttpError::Unreachable(last))
}

/// Header block and body split at the first blank line.
fn split_head(raw: &[u8]) -> Option<(&[u8], &[u8])> {
    let end = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    Some((&raw[..end], &raw[end + 4..]))
}

/// Header `name` (case-insensitive) from a header block.
fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().skip(1).find_map(|line| {
        let (k, v) = line.split_once(':')?;
        k.trim().eq_ignore_ascii_case(name).then(|| v.trim())
    })
}

/// Whether `raw` already holds the whole response (so a server that keeps
/// the connection open despite `Connection: close` doesn't stall us).
fn complete(raw: &[u8]) -> bool {
    let Some((head, body)) = split_head(raw) else {
        return false;
    };
    let head = String::from_utf8_lossy(head);
    if let Some(len) = header(&head, "content-length").and_then(|v| v.parse::<usize>().ok()) {
        return body.len() >= len;
    }
    if header(&head, "transfer-encoding").is_some_and(|v| v.eq_ignore_ascii_case("chunked")) {
        return body.ends_with(b"0\r\n\r\n");
    }
    false
}

fn parse_response(raw: &[u8]) -> Option<Response> {
    let (head, body) = split_head(raw)?;
    let head = String::from_utf8_lossy(head);
    let status_line = head.lines().next()?;
    let mut parts = status_line.split_whitespace();
    if !parts.next()?.starts_with("HTTP/") {
        return None;
    }
    let status = parts.next()?.parse::<u16>().ok()?;
    let body = if header(&head, "transfer-encoding")
        .is_some_and(|v| v.eq_ignore_ascii_case("chunked"))
    {
        dechunk(body)?
    } else if let Some(len) = header(&head, "content-length").and_then(|v| v.parse::<usize>().ok())
    {
        body.get(..len)?.to_vec()
    } else {
        body.to_vec()
    };
    Some(Response { status, body })
}

fn dechunk(mut body: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let line_end = body.windows(2).position(|w| w == b"\r\n")?;
        let size_text = std::str::from_utf8(&body[..line_end]).ok()?;
        let size_text = size_text.split(';').next()?.trim();
        let size = usize::from_str_radix(size_text, 16).ok()?;
        body = &body[line_end + 2..];
        if size == 0 {
            return Some(out);
        }
        out.extend_from_slice(body.get(..size)?);
        body = body.get(size + 2..)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comfyui::testing::{FakeHttp, Route, dead_port};

    #[test]
    fn parse_url_accepts_http_https_ports_and_prefixes() {
        let u = parse_url("http://127.0.0.1:8188").unwrap();
        assert_eq!(
            (u.https, u.host.as_str(), u.port, u.prefix.as_str()),
            (false, "127.0.0.1", 8188, "")
        );
        let u = parse_url("https://gpu.example.com/comfy/").unwrap();
        assert_eq!(
            (u.https, u.host.as_str(), u.port, u.prefix.as_str()),
            (true, "gpu.example.com", 443, "/comfy")
        );
        let u = parse_url("HTTP://[::1]:9000/a/b").unwrap();
        assert_eq!(
            (u.host.as_str(), u.port, u.prefix.as_str()),
            ("::1", 9000, "/a/b")
        );
        assert_eq!(parse_url("http://box").unwrap().port, 80);
    }

    #[test]
    fn parse_url_rejects_non_urls() {
        for bad in [
            "",
            "127.0.0.1:8188",
            "ftp://host",
            "http://",
            "http://host:abc",
            "http://host:0",
            "http://host:99999",
            "http://a b",
            "http://user@host",
            "http://::1:80",
        ] {
            assert_eq!(parse_url(bad), None, "{bad:?} should be rejected");
        }
    }

    #[test]
    fn get_reads_status_and_body_below_the_prefix() {
        let server = FakeHttp::start(vec![Route::json("/comfy/system_stats", 200, "{\"a\":1}")]);
        let url = parse_url(&format!("http://127.0.0.1:{}/comfy", server.port)).unwrap();
        let res = get(&url, "/system_stats", Duration::from_secs(2)).unwrap();
        assert_eq!(res.status, 200);
        assert_eq!(res.body, b"{\"a\":1}");
    }

    #[test]
    fn get_decodes_chunked_bodies() {
        let server = FakeHttp::start(vec![Route::raw(
            "/x",
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n",
        )]);
        let res = get(&Url::local(server.port), "/x", Duration::from_secs(2)).unwrap();
        assert_eq!(res.body, b"abcde");
    }

    #[test]
    fn get_without_server_is_unreachable() {
        let err = get(&Url::local(dead_port()), "/", Duration::from_secs(2)).unwrap_err();
        assert!(matches!(err, HttpError::Unreachable(_)));
    }

    #[test]
    fn get_times_out_on_a_silent_server() {
        let server = FakeHttp::silent();
        let started = Instant::now();
        let err = get(&Url::local(server.port), "/", Duration::from_millis(300)).unwrap_err();
        assert!(matches!(err, HttpError::Unreachable(_)));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn get_https_is_unsupported() {
        let url = parse_url("https://127.0.0.1:1").unwrap();
        assert_eq!(
            get(&url, "/", Duration::from_secs(1)),
            Err(HttpError::Unsupported)
        );
    }
}
