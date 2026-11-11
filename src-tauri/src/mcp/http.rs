//! The HTTP/1.1 framing the endpoint needs: one request at a time per
//! connection (keep-alive), `Content-Length` or chunked bodies, JSON
//! responses with a `Content-Length`. Clients are local MCP clients; there
//! is no TLS, no streaming response and no upgrade.

use std::io::{ErrorKind, Read, Write};
use std::time::{Duration, Instant};

/// Largest request head (request line and headers).
const MAX_HEAD: usize = 64 * 1024;
/// Largest request body: tool arguments are JSON, but `draw.pixels` and
/// `tile.set_pixels` can list many pixels.
const MAX_BODY: usize = 64 * 1024 * 1024;

/// One parsed request.
#[derive(Debug)]
pub(crate) struct Request {
    pub method: String,
    /// The target without its query.
    pub path: String,
    /// Header names in lower case, in arrival order.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// The client keeps the connection open after the response.
    pub keep_alive: bool,
}

impl Request {
    /// The first header named `name` (lower case).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Why no request was read.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ReadError {
    /// The client closed the connection or stayed idle too long.
    Closed,
    /// The endpoint is stopping.
    Stopped,
    /// Not an HTTP/1.x request this endpoint understands.
    Bad,
    /// Head or body over the limit.
    TooLarge,
}

/// Read the next request from `stream`, keeping bytes of a pipelined next
/// request in `buf`. The stream has a short read timeout; between reads
/// `stopped` is polled, and a connection without a byte for `idle` closes.
pub(crate) fn read_request<S: Read + Write>(
    stream: &mut S,
    buf: &mut Vec<u8>,
    stopped: &dyn Fn() -> bool,
    idle: Duration,
) -> Result<Request, ReadError> {
    let mut last_byte = Instant::now();
    let head_end = loop {
        if let Some(i) = find(buf, b"\r\n\r\n") {
            break i;
        }
        if buf.len() > MAX_HEAD {
            return Err(ReadError::TooLarge);
        }
        fill(stream, buf, stopped, idle, &mut last_byte)?;
    };
    let head = std::str::from_utf8(&buf[..head_end]).map_err(|_| ReadError::Bad)?;
    let mut lines = head.split("\r\n");
    let mut parts = lines.next().unwrap_or_default().split(' ');
    let (method, target, version) = match (parts.next(), parts.next(), parts.next()) {
        (Some(m), Some(t), Some(v)) if v.starts_with("HTTP/1.") => (m, t, v),
        _ => return Err(ReadError::Bad),
    };
    let mut headers = Vec::new();
    for line in lines {
        let (name, value) = line.split_once(':').ok_or(ReadError::Bad)?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
    }
    let mut req = Request {
        method: method.to_owned(),
        path: target.split('?').next().unwrap_or_default().to_owned(),
        headers,
        body: Vec::new(),
        keep_alive: version != "HTTP/1.0",
    };
    if let Some(c) = req.header("connection") {
        let c = c.to_ascii_lowercase();
        if c.contains("close") {
            req.keep_alive = false;
        } else if c.contains("keep-alive") {
            req.keep_alive = true;
        }
    }
    let body_start = head_end + 4;
    if req
        .header("expect")
        .is_some_and(|e| e.eq_ignore_ascii_case("100-continue"))
    {
        let _ = stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n");
    }
    let chunked = req
        .header("transfer-encoding")
        .is_some_and(|t| t.to_ascii_lowercase().contains("chunked"));
    if chunked {
        loop {
            match dechunk(&buf[body_start..])? {
                Some((body, used)) => {
                    req.body = body;
                    buf.drain(..body_start + used);
                    break;
                }
                None => fill(stream, buf, stopped, idle, &mut last_byte)?,
            }
        }
    } else {
        let len: usize = match req.header("content-length") {
            Some(v) => v.parse().map_err(|_| ReadError::Bad)?,
            None => 0,
        };
        if len > MAX_BODY {
            return Err(ReadError::TooLarge);
        }
        while buf.len() < body_start + len {
            fill(stream, buf, stopped, idle, &mut last_byte)?;
        }
        req.body = buf[body_start..body_start + len].to_vec();
        buf.drain(..body_start + len);
    }
    Ok(req)
}

/// Read more bytes into `buf`, waiting through read timeouts.
fn fill<S: Read>(
    stream: &mut S,
    buf: &mut Vec<u8>,
    stopped: &dyn Fn() -> bool,
    idle: Duration,
    last_byte: &mut Instant,
) -> Result<(), ReadError> {
    let mut chunk = [0u8; 16 * 1024];
    loop {
        if stopped() {
            return Err(ReadError::Stopped);
        }
        match stream.read(&mut chunk) {
            Ok(0) => return Err(ReadError::Closed),
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                *last_byte = Instant::now();
                return Ok(());
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                if last_byte.elapsed() >= idle {
                    return Err(ReadError::Closed);
                }
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => return Err(ReadError::Closed),
        }
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Decode a chunked body from the start of `data`: the body and the bytes
/// used, or `None` while it is incomplete.
fn dechunk(data: &[u8]) -> Result<Option<(Vec<u8>, usize)>, ReadError> {
    let mut body = Vec::new();
    let mut at = 0;
    loop {
        let Some(line_end) = find(&data[at..], b"\r\n") else {
            return Ok(None);
        };
        let line = std::str::from_utf8(&data[at..at + line_end]).map_err(|_| ReadError::Bad)?;
        let size_text = line.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_text, 16).map_err(|_| ReadError::Bad)?;
        at += line_end + 2;
        if size == 0 {
            // Optional trailers, then an empty line.
            let end = if data[at..].starts_with(b"\r\n") {
                Some(at + 2)
            } else {
                find(&data[at..], b"\r\n\r\n").map(|j| at + j + 4)
            };
            return Ok(end.map(|end| (body, end)));
        }
        if body.len() + size > MAX_BODY {
            return Err(ReadError::TooLarge);
        }
        if data.len() < at + size + 2 {
            return Ok(None);
        }
        body.extend_from_slice(&data[at..at + size]);
        at += size + 2;
    }
}

/// One response.
#[derive(Debug)]
pub(crate) struct Response {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

impl Response {
    /// A JSON response.
    pub fn json(status: u16, body: &serde_json::Value) -> Self {
        Self {
            status,
            headers: vec![("Content-Type", "application/json".to_owned())],
            body: body.to_string().into_bytes(),
        }
    }

    /// A response without a body.
    pub fn empty(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// The same response with one more header.
    pub fn with(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        _ => "Error",
    }
}

/// Write `res`; `keep_alive` decides the `Connection` header.
pub(crate) fn write_response<W: Write>(
    stream: &mut W,
    res: &Response,
    keep_alive: bool,
) -> std::io::Result<()> {
    let mut head = format!("HTTP/1.1 {} {}\r\n", res.status, reason(res.status));
    for (name, value) in &res.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!("Content-Length: {}\r\n", res.body.len()));
    head.push_str(if keep_alive {
        "Connection: keep-alive\r\n\r\n"
    } else {
        "Connection: close\r\n\r\n"
    });
    stream.write_all(head.as_bytes())?;
    stream.write_all(&res.body)?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// A stream that serves `input` and records what is written.
    struct Duplex {
        input: Cursor<Vec<u8>>,
        output: Vec<u8>,
    }

    impl Read for Duplex {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.input.read(buf)
        }
    }

    impl Write for Duplex {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.output.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn read_all(input: &str) -> (Vec<Result<Request, ReadError>>, Vec<u8>) {
        let mut s = Duplex {
            input: Cursor::new(input.as_bytes().to_vec()),
            output: Vec::new(),
        };
        let mut buf = Vec::new();
        let mut out = Vec::new();
        loop {
            let r = read_request(&mut s, &mut buf, &|| false, Duration::from_secs(1));
            let done = r.is_err();
            out.push(r);
            if done {
                return (out, s.output);
            }
        }
    }

    #[test]
    fn read_request_reads_pipelined_requests_and_chunked_bodies() {
        let (reqs, _) = read_all(
            "POST /mcp?x=1 HTTP/1.1\r\nContent-Length: 2\r\nAUTHORIZATION: Bearer t\r\n\r\n{}\
             POST /mcp HTTP/1.1\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3\r\nabc\r\n2;x=y\r\nde\r\n0\r\n\r\n",
        );
        let first = reqs[0].as_ref().unwrap();
        assert_eq!(
            (first.method.as_str(), first.path.as_str()),
            ("POST", "/mcp")
        );
        assert_eq!(first.header("authorization"), Some("Bearer t"));
        assert_eq!(first.body, b"{}");
        assert!(first.keep_alive);
        let second = reqs[1].as_ref().unwrap();
        assert_eq!(second.body, b"abcde");
        assert!(!second.keep_alive);
        assert_eq!(reqs[2].as_ref().unwrap_err(), &ReadError::Closed);
    }

    #[test]
    fn read_request_answers_expect_continue_and_refuses_garbage() {
        let (reqs, written) =
            read_all("POST /mcp HTTP/1.1\r\nExpect: 100-continue\r\nContent-Length: 1\r\n\r\nx");
        assert!(reqs[0].is_ok());
        assert!(written.starts_with(b"HTTP/1.1 100 Continue"));
        let (reqs, _) = read_all("NOT HTTP\r\n\r\n");
        assert_eq!(reqs[0].as_ref().unwrap_err(), &ReadError::Bad);
        let (reqs, _) = read_all("POST /mcp HTTP/1.1\r\nContent-Length: 999999999999\r\n\r\n");
        assert_eq!(reqs[0].as_ref().unwrap_err(), &ReadError::TooLarge);
    }

    #[test]
    fn write_response_sets_length_and_connection() {
        let mut out = Vec::new();
        let res = Response::json(200, &serde_json::json!({ "a": 1 })).with("Mcp-Session-Id", "s1");
        write_response(&mut out, &res, false).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(text.contains("Mcp-Session-Id: s1\r\n"));
        assert!(text.contains("Content-Length: 7\r\n"));
        assert!(text.ends_with("Connection: close\r\n\r\n{\"a\":1}"));
    }
}
