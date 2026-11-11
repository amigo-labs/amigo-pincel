// The native job backend's transport (docs/specs/performance-desktop-jobs.md,
// Decision Log 2026-11-11): a loopback HTTP endpoint the UI's job worker
// `fetch`es, so a job's bytes never cross Tauri's IPC (slow for bulk data on
// WebView2) or the page's main thread. One `POST /job`: the job as JSON in
// the `pincel-job` header, the document snapshot as the body, the result as
// the response body. Only this machine can connect (127.0.0.1), only with the
// random bearer token the page gets over `invoke`, and only from the app's
// own `Origin` and `Host` (DNS-rebinding guard). Plain `std::net`, one thread
// per connection; the job itself runs on rayon's pool.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;

use crate::native_jobs::{JOB_HEADER, NativeJob, NativeJobs};

/// Largest request head (request line and headers).
const MAX_HEAD: usize = 64 * 1024;
/// Largest request body: a snapshot of the biggest document the 32-bit
/// wasm core can hold.
const MAX_BODY: u64 = 4 * 1024 * 1024 * 1024;
/// A connection without a request for this long is closed.
const IDLE: Duration = Duration::from_secs(60);

/// Where the job worker sends jobs, and the token it must present.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct JobEndpoint {
    pub url: String,
    pub token: String,
}

/// The endpoint, or why it could not start (the UI then keeps every job in
/// the Web Worker).
pub struct JobEndpointState(pub Result<JobEndpoint, String>);

/// The endpoint the job worker fetches.
#[tauri::command]
pub fn job_endpoint(state: tauri::State<'_, JobEndpointState>) -> Result<JobEndpoint, String> {
    state.0.clone()
}

/// The `Origin`s of the app's own pages: Windows serves them from
/// `http://tauri.localhost`, macOS and Linux from `tauri://localhost`; a
/// debug build also from the Vite dev server.
fn app_origins() -> Vec<String> {
    let mut origins = vec![
        "http://tauri.localhost".to_string(),
        "https://tauri.localhost".to_string(),
        "tauri://localhost".to_string(),
    ];
    if cfg!(debug_assertions) {
        origins.push("http://localhost:5173".to_string());
    }
    origins
}

/// Binds a free port on 127.0.0.1 and serves jobs on `jobs` from a
/// background thread.
pub fn start(jobs: NativeJobs) -> std::io::Result<JobEndpoint> {
    start_with(jobs, app_origins())
}

fn start_with(jobs: NativeJobs, origins: Vec<String>) -> std::io::Result<JobEndpoint> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let port = listener.local_addr()?.port();
    let endpoint = JobEndpoint {
        url: format!("http://127.0.0.1:{port}/job"),
        token: new_token(),
    };
    let server = Arc::new(Server {
        jobs,
        token: endpoint.token.clone(),
        host: format!("127.0.0.1:{port}"),
        origins,
    });
    std::thread::Builder::new()
        .name("job-server".to_string())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let server = Arc::clone(&server);
                let _ = std::thread::Builder::new()
                    .name("job-connection".to_string())
                    .spawn(move || server.serve(stream));
            }
        })?;
    Ok(endpoint)
}

struct Server {
    jobs: NativeJobs,
    token: String,
    /// The only `Host` header accepted.
    host: String,
    origins: Vec<String>,
}

/// One parsed request head.
struct Head {
    method: String,
    path: String,
    /// Header names in lower case.
    headers: Vec<(String, String)>,
}

impl Head {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// What goes back to the client.
struct Response {
    status: u16,
    reason: &'static str,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
    /// Close the connection after this response.
    close: bool,
}

impl Response {
    fn text(status: u16, reason: &'static str, text: &str) -> Self {
        Self {
            status,
            reason,
            headers: vec![("Content-Type", "text/plain; charset=utf-8".to_string())],
            body: text.as_bytes().to_vec(),
            close: false,
        }
    }

    fn closing(mut self) -> Self {
        self.close = true;
        self
    }
}

impl Server {
    /// Answers requests on one connection until it closes.
    fn serve(&self, stream: TcpStream) {
        let _ = stream.set_read_timeout(Some(IDLE));
        let _ = stream.set_nodelay(true);
        let Ok(write_half) = stream.try_clone() else {
            return;
        };
        let mut reader = BufReader::with_capacity(256 * 1024, stream);
        let mut writer = write_half;
        loop {
            let head = match read_head(&mut reader) {
                Ok(Some(head)) => head,
                Ok(None) => return,
                Err(response) => {
                    let _ = write_response(&mut writer, None, response.closing());
                    return;
                }
            };
            let origin = head.header("origin").map(str::to_string);
            let response = self.respond(&head, &mut reader, &mut writer);
            let allowed = origin.filter(|o| self.origins.iter().any(|a| a == o));
            let close = response.close;
            if write_response(&mut writer, allowed.as_deref(), response).is_err() || close {
                return;
            }
        }
    }

    /// The response to `head`, reading its body from `reader`.
    fn respond<R: Read, W: Write>(&self, head: &Head, reader: &mut R, writer: &mut W) -> Response {
        if head.header("host") != Some(self.host.as_str()) {
            return Response::text(403, "Forbidden", "host not allowed").closing();
        }
        if let Some(origin) = head.header("origin")
            && !self.origins.iter().any(|o| o == origin)
        {
            return Response::text(403, "Forbidden", "origin not allowed").closing();
        }
        let length = match head.header("content-length").map(str::parse::<u64>) {
            None => 0,
            Some(Ok(n)) => n,
            Some(Err(_)) => return Response::text(400, "Bad Request", "bad length").closing(),
        };
        if head.header("transfer-encoding").is_some() {
            return Response::text(400, "Bad Request", "chunked bodies are not read").closing();
        }
        if length > MAX_BODY {
            return Response::text(413, "Payload Too Large", "body too large").closing();
        }
        match (head.method.as_str(), head.path.as_str()) {
            ("OPTIONS", "/job") => self.preflight(head),
            ("POST", "/job") => {
                if head
                    .header("expect")
                    .is_some_and(|e| e.eq_ignore_ascii_case("100-continue"))
                {
                    let _ = writer.write_all(b"HTTP/1.1 100 Continue\r\n\r\n");
                }
                // The body is read whatever the outcome, so the connection
                // stays usable; one `read_exact` into the whole buffer keeps
                // the reads large.
                let mut body = Vec::new();
                let read = usize::try_from(length).ok().and_then(|n| {
                    body.try_reserve_exact(n).ok()?;
                    body.resize(n, 0);
                    reader.read_exact(&mut body).ok()
                });
                if read.is_none() {
                    return Response::text(400, "Bad Request", "body cut short").closing();
                }
                self.run(head, &body)
            }
            (_, "/job") => Response::text(405, "Method Not Allowed", "use POST").closing(),
            _ => Response::text(404, "Not Found", "not found").closing(),
        }
    }

    /// The CORS preflight of the job worker's `fetch`.
    fn preflight(&self, head: &Head) -> Response {
        let mut response = Response {
            status: 204,
            reason: "No Content",
            headers: vec![
                ("Access-Control-Allow-Methods", "POST".to_string()),
                (
                    "Access-Control-Allow-Headers",
                    format!("authorization, content-type, {JOB_HEADER}"),
                ),
                ("Access-Control-Max-Age", "600".to_string()),
            ],
            body: Vec::new(),
            close: false,
        };
        if head.header("access-control-request-private-network") == Some("true") {
            response
                .headers
                .push(("Access-Control-Allow-Private-Network", "true".to_string()));
        }
        response
    }

    /// Runs the job of an authorized `POST /job`.
    fn run(&self, head: &Head, body: &[u8]) -> Response {
        let presented = head
            .header("authorization")
            .and_then(|a| a.strip_prefix("Bearer "))
            .unwrap_or_default();
        if !token_eq(presented, &self.token) {
            return Response::text(401, "Unauthorized", "missing or wrong token");
        }
        let Some(job) = head.header(JOB_HEADER) else {
            return Response::text(400, "Bad Request", "the job header is missing");
        };
        let job: NativeJob = match serde_json::from_str(job) {
            Ok(job) => job,
            Err(e) => return Response::text(400, "Bad Request", &e.to_string()),
        };
        match self.jobs.run(job, body) {
            Ok(bytes) => Response {
                status: 200,
                reason: "OK",
                headers: vec![("Content-Type", "application/octet-stream".to_string())],
                body: bytes,
                close: false,
            },
            Err(e) => Response::text(500, "Internal Server Error", &e),
        }
    }
}

/// Reads a request head; `Ok(None)` when the client closed or went idle.
fn read_head<R: BufRead>(reader: &mut R) -> Result<Option<Head>, Response> {
    let bad = || Response::text(400, "Bad Request", "malformed request");
    let mut lines = Vec::new();
    let mut size = 0;
    loop {
        let mut line = Vec::new();
        let n = (&mut *reader)
            .take((MAX_HEAD - size + 1) as u64)
            .read_until(b'\n', &mut line)
            .map_err(|_| bad())?;
        if n == 0 {
            if lines.is_empty() {
                return Ok(None);
            }
            return Err(bad());
        }
        size += n;
        if size > MAX_HEAD {
            return Err(Response::text(
                431,
                "Request Header Fields Too Large",
                "head too large",
            ));
        }
        let line = String::from_utf8(line).map_err(|_| bad())?;
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            if lines.is_empty() {
                continue; // tolerate a stray CRLF between requests
            }
            break;
        }
        lines.push(line.to_string());
    }
    let mut parts = lines[0].split(' ');
    let (method, target) = match (parts.next(), parts.next(), parts.next()) {
        (Some(m), Some(t), Some(v)) if v.starts_with("HTTP/1.") => (m, t),
        _ => return Err(bad()),
    };
    let mut headers = Vec::new();
    for line in &lines[1..] {
        let (name, value) = line.split_once(':').ok_or_else(bad)?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
    }
    Ok(Some(Head {
        method: method.to_string(),
        path: target.split('?').next().unwrap_or_default().to_string(),
        headers,
    }))
}

fn write_response<W: Write>(
    writer: &mut W,
    origin: Option<&str>,
    response: Response,
) -> std::io::Result<()> {
    let mut head = format!("HTTP/1.1 {} {}\r\n", response.status, response.reason);
    for (name, value) in &response.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    if let Some(origin) = origin {
        head.push_str(&format!(
            "Access-Control-Allow-Origin: {origin}\r\nVary: Origin\r\n"
        ));
    }
    head.push_str(&format!(
        "Content-Length: {}\r\nConnection: {}\r\n\r\n",
        response.body.len(),
        if response.close {
            "close"
        } else {
            "keep-alive"
        }
    ));
    writer.write_all(head.as_bytes())?;
    writer.write_all(&response.body)?;
    writer.flush()
}

/// 64 random bits: every thread seeds its `RandomState` keys from the
/// operating system once, and SipHash under secret keys is a pseudorandom
/// function, so a fresh thread per word gives independent words without a
/// dependency for the RNG.
fn os_random_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let word = || {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(0x6a6f_6273_6572_7665);
        h.finish()
    };
    std::thread::spawn(word).join().unwrap_or_else(|_| word())
}

/// A new 256-bit bearer token as lower-case hex.
fn new_token() -> String {
    (0..4)
        .map(|_| format!("{:016x}", os_random_u64()))
        .collect()
}

/// Compares a presented token with the known one without an early exit.
fn token_eq(presented: &str, known: &str) -> bool {
    let (a, b) = (presented.as_bytes(), known.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use pincel_image_core::codec::snapshot::to_snapshot_bytes;
    use pincel_image_core::document::Document;
    use pincel_image_core::jobs::{self, EncodeFormat};

    const ORIGIN: &str = "http://tauri.localhost";

    fn server() -> JobEndpoint {
        start_with(NativeJobs::default(), vec![ORIGIN.to_string()]).expect("server")
    }

    fn host(endpoint: &JobEndpoint) -> &str {
        endpoint
            .url
            .trim_start_matches("http://")
            .trim_end_matches("/job")
    }

    /// Sends `head` (CRLF-joined lines) and `body`; returns the status,
    /// the lower-case headers and the body of the response.
    fn send(
        endpoint: &JobEndpoint,
        head: &[String],
        body: &[u8],
    ) -> (u16, Vec<(String, String)>, Vec<u8>) {
        let mut stream = TcpStream::connect(host(endpoint)).expect("connect");
        let mut request = head.join("\r\n").into_bytes();
        request.extend_from_slice(b"\r\n\r\n");
        request.extend_from_slice(body);
        stream.write_all(&request).expect("send");
        let mut reader = BufReader::new(stream);
        let mut status_line = String::new();
        reader.read_line(&mut status_line).expect("status");
        let status = status_line
            .split(' ')
            .nth(1)
            .and_then(|s| s.parse().ok())
            .expect("code");
        let mut headers = Vec::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("header");
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            let (n, v) = line.split_once(':').expect("header line");
            headers.push((n.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
        let len: usize = headers
            .iter()
            .find(|(n, _)| n == "content-length")
            .and_then(|(_, v)| v.parse().ok())
            .expect("length");
        let mut body = vec![0u8; len];
        reader.read_exact(&mut body).expect("body");
        (status, headers, body)
    }

    fn post(endpoint: &JobEndpoint, extra: &[String], job: &str, body: &[u8]) -> Vec<String> {
        let mut head = vec![
            "POST /job HTTP/1.1".to_string(),
            format!("Host: {}", host(endpoint)),
            format!("Content-Length: {}", body.len()),
            format!("{JOB_HEADER}: {job}"),
        ];
        head.extend_from_slice(extra);
        head
    }

    fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
        headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    const PNG_JOB: &str = r#"{"kind":"encode","format":"png","level":6,"quality":90}"#;

    #[test]
    fn post_job_with_the_token_answers_the_job_result() {
        let endpoint = server();
        let doc = Document::new(8, 4).expect("doc");
        let snapshot = to_snapshot_bytes(&doc);
        let auth = vec![
            format!("Authorization: Bearer {}", endpoint.token),
            format!("Origin: {ORIGIN}"),
        ];
        let (status, headers, body) = send(
            &endpoint,
            &post(&endpoint, &auth, PNG_JOB, &snapshot),
            &snapshot,
        );
        assert_eq!(status, 200);
        assert_eq!(
            body,
            jobs::encode(&doc, EncodeFormat::Png, 6, 90).expect("png")
        );
        assert_eq!(
            header(&headers, "access-control-allow-origin"),
            Some(ORIGIN)
        );
        // A failing job answers its error, readable by the page.
        let (status, headers, body) = send(
            &endpoint,
            &post(&endpoint, &auth, PNG_JOB, b"junk"),
            b"junk",
        );
        assert_eq!(status, 500);
        assert!(!body.is_empty());
        assert_eq!(
            header(&headers, "access-control-allow-origin"),
            Some(ORIGIN)
        );
    }

    #[test]
    fn post_job_without_the_right_token_is_refused() {
        let endpoint = server();
        for auth in [
            vec![],
            vec!["Authorization: Bearer 0123".to_string()],
            vec![format!("Authorization: Basic {}", endpoint.token)],
        ] {
            let (status, _, _) = send(&endpoint, &post(&endpoint, &auth, PNG_JOB, b""), b"");
            assert_eq!(status, 401, "{auth:?}");
        }
    }

    #[test]
    fn requests_from_a_foreign_origin_or_host_are_refused() {
        let endpoint = server();
        let token = format!("Authorization: Bearer {}", endpoint.token);
        let foreign = vec![token.clone(), "Origin: https://evil.example".to_string()];
        let (status, headers, _) = send(&endpoint, &post(&endpoint, &foreign, PNG_JOB, b""), b"");
        assert_eq!(status, 403);
        assert_eq!(header(&headers, "access-control-allow-origin"), None);
        // DNS rebinding: a page of another name that resolves to 127.0.0.1.
        let rebound = vec![
            "POST /job HTTP/1.1".to_string(),
            "Host: evil.example:80".to_string(),
            "Content-Length: 0".to_string(),
            token,
        ];
        assert_eq!(send(&endpoint, &rebound, b"").0, 403);
        let preflight = vec![
            "OPTIONS /job HTTP/1.1".to_string(),
            format!("Host: {}", host(&endpoint)),
            "Origin: https://evil.example".to_string(),
        ];
        assert_eq!(send(&endpoint, &preflight, b"").0, 403);
    }

    #[test]
    fn preflight_from_the_app_allows_the_job_request() {
        let endpoint = server();
        let preflight = vec![
            "OPTIONS /job HTTP/1.1".to_string(),
            format!("Host: {}", host(&endpoint)),
            format!("Origin: {ORIGIN}"),
            "Access-Control-Request-Method: POST".to_string(),
            "Access-Control-Request-Private-Network: true".to_string(),
        ];
        let (status, headers, _) = send(&endpoint, &preflight, b"");
        assert_eq!(status, 204);
        assert_eq!(
            header(&headers, "access-control-allow-origin"),
            Some(ORIGIN)
        );
        assert_eq!(
            header(&headers, "access-control-allow-methods"),
            Some("POST")
        );
        let allowed = header(&headers, "access-control-allow-headers").unwrap_or_default();
        assert!(allowed.contains("authorization") && allowed.contains(JOB_HEADER));
        assert_eq!(
            header(&headers, "access-control-allow-private-network"),
            Some("true")
        );
    }

    #[test]
    fn other_paths_methods_and_malformed_requests_are_refused() {
        let endpoint = server();
        let h = format!("Host: {}", host(&endpoint));
        assert_eq!(
            send(
                &endpoint,
                &["GET /other HTTP/1.1".to_string(), h.clone()],
                b""
            )
            .0,
            404
        );
        assert_eq!(
            send(
                &endpoint,
                &["GET /job HTTP/1.1".to_string(), h.clone()],
                b""
            )
            .0,
            405
        );
        assert_eq!(
            send(&endpoint, &["NONSENSE".to_string(), h.clone()], b"").0,
            400
        );
        let huge = vec![
            "POST /job HTTP/1.1".to_string(),
            h,
            format!("Content-Length: {}", MAX_BODY + 1),
        ];
        assert_eq!(send(&endpoint, &huge, b"").0, 413);
    }

    #[test]
    fn new_token_is_256_random_bits() {
        let (a, b) = (new_token(), new_token());
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
        assert!(token_eq(&a, &a.clone()));
        assert!(!token_eq(&a, &b));
        assert!(!token_eq(&a, &a[..63]));
    }
}
