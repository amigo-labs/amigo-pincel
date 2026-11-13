//! ComfyUI's progress WebSocket (`/ws?clientId=…`) through the synchronous
//! tungstenite API that `tokio-tungstenite` re-exports: the generation
//! threads block, so a plain TCP stream with a read timeout is enough.
//! `https` servers get no socket; their jobs are polled (Decisions).

use std::io::ErrorKind;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use tokio_tungstenite::tungstenite::{self, Message, WebSocket};

use super::super::http::Url;

/// One message of the socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// A JSON event (`status`, `progress`, `executing`, …).
    Text(String),
    /// A binary event, e.g. a preview image.
    Binary(Vec<u8>),
}

/// The socket closed or broke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Closed;

/// An open progress socket.
pub struct Socket {
    ws: WebSocket<TcpStream>,
}

impl Socket {
    /// Connects as `client_id`; `None` for an `https` server or when the
    /// server does not accept the socket. `wait` bounds the connect and
    /// the handshake; a read returns after `poll` without a message.
    pub fn connect(url: &Url, client_id: &str, wait: Duration, poll: Duration) -> Option<Self> {
        if url.https {
            return None;
        }
        let addr = (url.host.as_str(), url.port)
            .to_socket_addrs()
            .ok()?
            .next()?;
        let stream = TcpStream::connect_timeout(&addr, wait).ok()?;
        stream.set_read_timeout(Some(wait)).ok()?;
        stream.set_write_timeout(Some(wait)).ok()?;
        let host = if url.host.contains(':') {
            format!("[{}]", url.host)
        } else {
            url.host.clone()
        };
        let uri = format!(
            "ws://{host}:{}{}/ws?clientId={client_id}",
            url.port, url.prefix
        );
        let (ws, _) = tungstenite::client(uri.as_str(), stream).ok()?;
        ws.get_ref().set_read_timeout(Some(poll)).ok()?;
        Some(Self { ws })
    }

    /// The next message; `Ok(None)` when none arrived within the poll time.
    pub fn next(&mut self) -> Result<Option<Frame>, Closed> {
        match self.ws.read() {
            Ok(Message::Text(text)) => Ok(Some(Frame::Text(text.as_str().to_string()))),
            Ok(Message::Binary(bytes)) => Ok(Some(Frame::Binary(bytes.to_vec()))),
            Ok(Message::Close(_)) => Err(Closed),
            Ok(_) => Ok(None),
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                Ok(None)
            }
            Err(_) => Err(Closed),
        }
    }
}
