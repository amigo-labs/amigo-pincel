//! HTTP(S) GET for ComfyUI's JSON API through `reqwest`, with rustls and
//! the ring provider installed process-wide (Decision Log 2026-11-11).
//! Callers are blocking threads; each request runs on Tauri's async runtime.

use std::net::IpAddr;
use std::sync::OnceLock;
use std::time::Duration;

/// A server address: scheme, host, port and an optional path prefix (a
/// reverse proxy may serve ComfyUI below `/comfy`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
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

    /// Whether the host is this machine (`localhost`, 127.0.0.0/8, `::1`).
    pub fn is_loopback(&self) -> bool {
        self.host.eq_ignore_ascii_case("localhost")
            || self.host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
    }

    /// The URL as text, e.g. `http://127.0.0.1:8188/comfy`; the scheme's
    /// default port is left out.
    pub fn display(&self) -> String {
        let scheme = if self.https { "https" } else { "http" };
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        let default_port = if self.https { 443 } else { 80 };
        if self.port == default_port {
            format!("{scheme}://{host}{}", self.prefix)
        } else {
            format!("{scheme}://{host}:{}{}", self.port, self.prefix)
        }
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

/// A response: status code and body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Why a request got no HTTP response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    /// No connection, a failed TLS handshake, a timeout, or an answer that
    /// is not HTTP.
    Unreachable(String),
}

/// Installs rustls' ring provider as the process-wide default, once. The
/// `reqwest` in this tree is built without a provider of its own and
/// panics when a client is built before one is installed.
pub fn install_crypto_provider() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            // Losing a race to another installer leaves a provider in place.
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
    });
}

/// The shared client: loopback requests bypass the system proxy, which
/// would otherwise see (or block) traffic to this machine.
fn client(loopback: bool) -> Result<&'static reqwest::Client, HttpError> {
    static LOCAL: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    static REMOTE: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    let cell = if loopback { &LOCAL } else { &REMOTE };
    cell.get_or_init(|| {
        install_crypto_provider();
        let builder = reqwest::Client::builder().user_agent("Pincel");
        let builder = if loopback {
            builder.no_proxy()
        } else {
            builder
        };
        builder.build().map_err(|e| e.to_string())
    })
    .as_ref()
    .map_err(|e| HttpError::Unreachable(e.clone()))
}

fn unreachable(e: reqwest::Error) -> HttpError {
    HttpError::Unreachable(e.to_string())
}

/// `GET <prefix><path>` with `timeout` for the whole exchange. Blocks the
/// calling thread, which must not be an async task.
pub fn get(url: &Url, path: &str, timeout: Duration) -> Result<Response, HttpError> {
    let request = client(url.is_loopback())?
        .get(format!("{}{path}", url.display()))
        .header(reqwest::header::ACCEPT, "application/json")
        .timeout(timeout);
    tauri::async_runtime::block_on(async move {
        let response = request.send().await.map_err(unreachable)?;
        let status = response.status().as_u16();
        let body = response.bytes().await.map_err(unreachable)?.to_vec();
        Ok(Response { status, body })
    })
}

/// `POST <prefix><path>` with `body` of `content_type` and `timeout` for
/// the whole exchange. Blocks like [`get`].
pub fn post(
    url: &Url,
    path: &str,
    content_type: &str,
    body: Vec<u8>,
    timeout: Duration,
) -> Result<Response, HttpError> {
    let request = client(url.is_loopback())?
        .post(format!("{}{path}", url.display()))
        .header(reqwest::header::CONTENT_TYPE, content_type)
        .body(body)
        .timeout(timeout);
    tauri::async_runtime::block_on(async move {
        let response = request.send().await.map_err(unreachable)?;
        let status = response.status().as_u16();
        let body = response.bytes().await.map_err(unreachable)?.to_vec();
        Ok(Response { status, body })
    })
}

fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    use crate::comfyui::testing::{FakeHttp, RecordingTcp, Route, dead_port};

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
    fn is_loopback_only_for_this_machine() {
        assert!(parse_url("http://localhost:1").unwrap().is_loopback());
        assert!(parse_url("http://127.0.0.2:1").unwrap().is_loopback());
        assert!(parse_url("http://[::1]:1").unwrap().is_loopback());
        assert!(!parse_url("http://192.168.1.5:1").unwrap().is_loopback());
        assert!(!parse_url("https://gpu.example.com").unwrap().is_loopback());
    }

    #[test]
    fn display_leaves_out_the_default_port() {
        assert_eq!(
            parse_url("https://gpu.example.com/comfy/")
                .unwrap()
                .display(),
            "https://gpu.example.com/comfy"
        );
        assert_eq!(
            parse_url("http://[::1]:8188").unwrap().display(),
            "http://[::1]:8188"
        );
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
    fn get_https_speaks_tls_and_a_failed_handshake_is_unreachable() {
        let server = RecordingTcp::start();
        let url = parse_url(&format!("https://127.0.0.1:{}", server.port)).unwrap();
        let err = get(&url, "/system_stats", Duration::from_secs(2)).unwrap_err();
        assert!(matches!(err, HttpError::Unreachable(_)), "{err:?}");
        let first = server.first.lock().unwrap().clone();
        // A TLS handshake record (ClientHello), not a plain GET.
        assert_eq!(first.get(..2), Some(&[0x16, 0x03][..]), "{first:?}");
    }

    #[test]
    fn get_https_to_a_plain_http_server_is_unreachable() {
        let server = FakeHttp::comfy("", "{}", "{}");
        let url = parse_url(&format!("https://127.0.0.1:{}", server.port)).unwrap();
        let err = get(&url, "/system_stats", Duration::from_secs(2)).unwrap_err();
        assert!(matches!(err, HttpError::Unreachable(_)), "{err:?}");
    }
}
