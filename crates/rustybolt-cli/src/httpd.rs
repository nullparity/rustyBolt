//! Reads HTTP/1.x requests from a socket with `httparse`.
//!
//! Both local servers (the launcher API and the consent catcher) share this
//! reader, so request limits and parsing live in one place.

use std::io::{self, ErrorKind, Read};
use std::net::TcpStream;

/// The most bytes a request head (request line and headers) may take.
const MAX_HEAD_BYTES: usize = 16 * 1024;
/// The most bytes a request body may take.
const MAX_BODY_BYTES: usize = 1024 * 1024;
/// The most headers a request may carry.
const MAX_HEADERS: usize = 64;

/// One parsed request. Header names are matched without case.
#[derive(Debug)]
pub(crate) struct Request {
    pub method: String,
    pub path: String,
    /// Empty when the target has no `?`.
    pub query: String,
    pub body: Vec<u8>,
    /// The client asked to keep the connection open.
    pub keep_alive: bool,
    /// Every header, with the name in lower case.
    pub headers: Vec<(String, String)>,
}

impl Request {
    /// The value of the first header with this name, matched without case.
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(held, _)| held == &name.to_ascii_lowercase())
            .map(|(_, value)| value.as_str())
    }
}

/// A connection that yields requests in order.
pub(crate) struct Connection {
    stream: TcpStream,
    /// Bytes read past the end of the last request.
    carry: Vec<u8>,
}

impl Connection {
    pub(crate) fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            carry: Vec::new(),
        }
    }

    /// Reads the next request. `Ok(None)` means the peer closed the socket
    /// before a request started.
    pub(crate) fn next_request(&mut self) -> io::Result<Option<Request>> {
        let mut raw = std::mem::take(&mut self.carry);
        let mut chunk = [0u8; 4096];

        let (head_len, request) = loop {
            let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
            let mut parsed = httparse::Request::new(&mut headers);
            match parsed.parse(&raw) {
                Ok(httparse::Status::Complete(head_len)) => {
                    let request = Request::from_head(&parsed);
                    break (head_len, request);
                }
                Ok(httparse::Status::Partial) => {}
                Err(error) => return Err(invalid(error.to_string())),
            }
            if raw.len() >= MAX_HEAD_BYTES {
                return Err(invalid("request head too large"));
            }
            let n = self.stream.read(&mut chunk)?;
            if n == 0 {
                return if raw.is_empty() {
                    Ok(None)
                } else {
                    Err(io::Error::new(
                        ErrorKind::UnexpectedEof,
                        "connection closed mid-request",
                    ))
                };
            }
            raw.extend_from_slice(&chunk[..n]);
        };

        let mut request = request?;
        let content_length = request.body.len();
        if content_length > MAX_BODY_BYTES {
            return Err(invalid("request body too large"));
        }
        let mut body = raw.split_off(head_len);
        while body.len() < content_length {
            let n = self.stream.read(&mut chunk)?;
            if n == 0 {
                return Err(io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "connection closed mid-body",
                ));
            }
            body.extend_from_slice(&chunk[..n]);
        }
        self.carry = body.split_off(content_length);
        request.body = body;
        Ok(Some(request))
    }

    pub(crate) fn stream(&mut self) -> &mut TcpStream {
        &mut self.stream
    }
}

impl Request {
    /// Builds a request from a complete head. `body` holds a placeholder of
    /// `Content-Length` bytes until the caller reads the real body.
    fn from_head(parsed: &httparse::Request<'_, '_>) -> io::Result<Request> {
        let method = parsed.method.unwrap_or("").to_string();
        let target = parsed.path.unwrap_or("");
        let (path, query) = target.split_once('?').unwrap_or((target, ""));

        let mut content_length = 0usize;
        let mut keep_alive = false;
        let mut headers = Vec::with_capacity(parsed.headers.len());
        for header in parsed.headers.iter() {
            let name = header.name.to_ascii_lowercase();
            let value = String::from_utf8_lossy(header.value).trim().to_string();
            if name == "content-length" {
                content_length = value.parse().map_err(|_| invalid("bad content-length"))?;
            } else if name == "connection" {
                keep_alive = value.to_ascii_lowercase().contains("keep-alive");
            }
            headers.push((name, value));
        }

        Ok(Request {
            method,
            path: path.to_string(),
            query: query.to_string(),
            body: vec![0; content_length],
            keep_alive,
            headers,
        })
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;

    fn serve(bytes: &'static [u8]) -> Connection {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let mut client = TcpStream::connect(addr).unwrap();
            client.write_all(bytes).unwrap();
        });
        Connection::new(listener.accept().unwrap().0)
    }

    #[test]
    fn reads_two_pipelined_requests_with_bodies() {
        let mut conn = serve(
            b"POST /a?x=1 HTTP/1.1\r\nContent-Length: 3\r\nConnection: keep-alive\r\n\r\nabcGET /b HTTP/1.1\r\n\r\n",
        );
        let first = conn.next_request().unwrap().unwrap();
        assert_eq!(first.method, "POST");
        assert_eq!(first.path, "/a");
        assert_eq!(first.query, "x=1");
        assert_eq!(first.body, b"abc");
        assert!(first.keep_alive);
        assert_eq!(first.header("Content-Length"), Some("3"));
        assert_eq!(first.header("x-missing"), None);
        let second = conn.next_request().unwrap().unwrap();
        assert_eq!(second.path, "/b");
        assert!(second.body.is_empty());
        assert!(!second.keep_alive);
        assert!(conn.next_request().unwrap().is_none());
    }

    #[test]
    fn rejects_a_bad_head() {
        let mut conn = serve(b"not http\r\n\r\n");
        assert_eq!(
            conn.next_request().unwrap_err().kind(),
            ErrorKind::InvalidData
        );
    }
}
