//! Minimal real loopback HTTP/SSE transport for contract integration tests.
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    time::Duration,
};
#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}
impl Response {
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).expect("JSON response")
    }
}
pub fn connect(
    port: u16,
    method: &str,
    path: &str,
    headers: &BTreeMap<String, String>,
    body: &[u8],
) -> TcpStream {
    let socket = TcpStream::connect(("127.0.0.1", port)).expect("connect server");
    send_request(socket, port, method, path, headers, body)
}
/// Connects with a fixed small receive buffer. Linux otherwise autotunes loopback buffers to
/// many megabytes, so a reader that stops reading would never hold the server back.
pub fn connect_small(port: u16, path: &str) -> TcpStream {
    use socket2::{Domain, Socket, Type};
    let socket = Socket::new(Domain::IPV4, Type::STREAM, None).expect("socket");
    socket.set_recv_buffer_size(4096).expect("receive buffer");
    socket
        .connect(&std::net::SocketAddr::from(([127, 0, 0, 1], port)).into())
        .expect("connect server");
    send_request(socket.into(), port, "GET", path, &BTreeMap::new(), &[])
}
fn send_request(
    mut socket: TcpStream,
    port: u16,
    method: &str,
    path: &str,
    headers: &BTreeMap<String, String>,
    body: &[u8],
) -> TcpStream {
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let host = headers
        .get("Host")
        .cloned()
        .unwrap_or_else(|| format!("127.0.0.1:{port}"));
    let mut request = format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n");
    for (name, value) in headers {
        if name != "Host" {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
    }
    if !headers
        .keys()
        .any(|key| key.eq_ignore_ascii_case("content-length"))
    {
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    request.push_str("\r\n");
    socket.write_all(request.as_bytes()).unwrap();
    socket.write_all(body).unwrap();
    socket
}
pub fn head(reader: &mut BufReader<TcpStream>) -> (u16, BTreeMap<String, String>) {
    let mut line = String::new();
    reader.read_line(&mut line).expect("HTTP status");
    let status = line
        .split_whitespace()
        .nth(1)
        .expect("status code")
        .parse()
        .unwrap();
    let mut headers = BTreeMap::new();
    loop {
        line.clear();
        reader.read_line(&mut line).expect("HTTP headers");
        if line == "\r\n" || line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':').expect("header separator");
        headers.insert(name.to_ascii_lowercase(), value.trim().to_string());
    }
    (status, headers)
}
pub fn request(
    port: u16,
    method: &str,
    path: &str,
    headers: &BTreeMap<String, String>,
    body: &[u8],
) -> Response {
    let mut reader = BufReader::new(connect(port, method, path, headers, body));
    let (status, headers) = head(&mut reader);
    let body = if method == "HEAD" || matches!(status, 100..=199 | 204 | 304) {
        Vec::new()
    } else {
        read_body(&mut reader, &headers)
    };
    Response {
        status,
        headers,
        body,
    }
}
fn read_body(reader: &mut impl BufRead, headers: &BTreeMap<String, String>) -> Vec<u8> {
    let mut output = Vec::new();
    if headers
        .get("transfer-encoding")
        .is_some_and(|s| s == "chunked")
    {
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("chunk length line");
            assert!(line.ends_with("\r\n"), "complete chunk length line");
            let size = usize::from_str_radix(line.trim().split(';').next().unwrap(), 16)
                .expect("hex chunk length");
            if size == 0 {
                loop {
                    line.clear();
                    reader.read_line(&mut line).expect("chunk trailer");
                    assert!(line.ends_with("\r\n"), "complete chunk trailer");
                    if line == "\r\n" {
                        return output;
                    }
                }
            }
            let start = output.len();
            output.resize(start + size, 0);
            reader.read_exact(&mut output[start..]).expect("HTTP chunk");
            let mut end = [0; 2];
            reader.read_exact(&mut end).expect("chunk terminator");
            assert_eq!(&end, b"\r\n");
        }
    } else if let Some(length) = headers.get("content-length") {
        output.resize(length.parse().expect("HTTP content length"), 0);
        reader.read_exact(&mut output).expect("HTTP body");
    } else {
        reader.read_to_end(&mut output).expect("HTTP body");
    }
    output
}
pub struct Events {
    body: EventBody<BufReader<TcpStream>>,
}
impl Events {
    pub fn open(port: u16, token: &str) -> Self {
        Self::open_path(port, &format!("/api/events?token={token}"))
    }
    pub fn open_path(port: u16, path: &str) -> Self {
        Self::from_socket(connect(port, "GET", path, &BTreeMap::new(), &[]))
    }
    /// An event stream whose reader can fall behind the server, as a stalled browser tab does.
    pub fn open_small(port: u16, token: &str) -> Self {
        Self::from_socket(connect_small(port, &format!("/api/events?token={token}")))
    }
    fn from_socket(socket: TcpStream) -> Self {
        let mut reader = BufReader::new(socket);
        let (status, headers) = head(&mut reader);
        assert_eq!(status, 200);
        assert_eq!(headers["content-type"], "text/event-stream");
        Self {
            body: EventBody {
                reader,
                chunked: headers
                    .get("transfer-encoding")
                    .is_some_and(|s| s == "chunked"),
                remaining: 0,
            },
        }
    }
    pub fn next(&mut self) -> Value {
        self.try_next().expect("SSE ended before expected event")
    }
    /// The next raw line, comments included, waiting up to `limit` for it.
    pub fn line_within(&mut self, limit: Duration) -> Option<String> {
        let timeout = |body: &EventBody<BufReader<TcpStream>>, limit| {
            body.reader
                .get_ref()
                .set_read_timeout(Some(limit))
                .expect("read timeout");
        };
        timeout(&self.body, limit);
        let line = self.body.line();
        timeout(&self.body, Duration::from_secs(5));
        line
    }
    /// The next event, or None once the server has ended the stream.
    pub fn try_next(&mut self) -> Option<Value> {
        loop {
            let line = self.body.line()?;
            if let Some(data) = line.strip_prefix("data: ") {
                return Some(serde_json::from_str(data.trim()).expect("SSE JSON"));
            }
        }
    }
}

/// Decodes transport chunks before reading SSE lines, preserving split UTF-8 bytes.
struct EventBody<R> {
    reader: R,
    chunked: bool,
    remaining: usize,
}
impl<R: BufRead> EventBody<R> {
    fn byte(&mut self) -> Option<u8> {
        if self.chunked && self.remaining == 0 {
            let mut line = String::new();
            self.reader.read_line(&mut line).expect("chunk length");
            self.remaining = usize::from_str_radix(line.trim().split(';').next().unwrap(), 16)
                .expect("hex chunk length");
            if self.remaining == 0 {
                return None;
            }
        }
        let mut byte = [0];
        if self.reader.read(&mut byte).expect("SSE body byte") == 0 {
            return None;
        }
        if self.chunked {
            self.remaining -= 1;
            if self.remaining == 0 {
                let mut end = [0; 2];
                self.reader.read_exact(&mut end).expect("chunk terminator");
                assert_eq!(&end, b"\r\n");
            }
        }
        Some(byte[0])
    }
    fn line(&mut self) -> Option<String> {
        let mut bytes = Vec::new();
        loop {
            match self.byte() {
                Some(b'\n') => return Some(String::from_utf8(bytes).expect("SSE UTF-8")),
                Some(byte) => bytes.push(byte),
                None if bytes.is_empty() => return None,
                None => return Some(String::from_utf8(bytes).expect("SSE UTF-8")),
            }
            assert!(bytes.len() <= 8_000_000, "bounded SSE line");
        }
    }
}
#[cfg(test)]
mod tests {
    use super::{EventBody, read_body};
    use std::{
        collections::BTreeMap,
        io::{self, BufReader, Cursor, Read},
    };
    struct ResetAfter(Cursor<Vec<u8>>);
    impl Read for ResetAfter {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            if self.0.position() == self.0.get_ref().len() as u64 {
                return Err(io::ErrorKind::ConnectionReset.into());
            }
            self.0.read(bytes)
        }
    }
    #[test]
    fn framed_http_bodies_finish_before_a_connection_reset() {
        for (header, value, wire) in [
            ("content-length", "5", b"hello".as_slice()),
            (
                "transfer-encoding",
                "chunked",
                b"2;fixture=yes\r\nhe\r\n3\r\nllo\r\n0\r\nTrailer: yes\r\n\r\n",
            ),
        ] {
            let headers = BTreeMap::from([(header.into(), value.into())]);
            let mut reader = BufReader::new(ResetAfter(Cursor::new(wire.to_vec())));
            assert_eq!(read_body(&mut reader, &headers), b"hello");
        }
    }
    #[test]
    fn incomplete_http_frames_are_rejected() {
        for (header, value, wire) in [
            ("content-length", "6", b"hello".as_slice()),
            ("transfer-encoding", "chunked", b"5\r\nhell"),
            ("transfer-encoding", "chunked", b"5\r\nhello\r\n0\r\n"),
        ] {
            assert!(
                std::panic::catch_unwind(|| {
                    read_body(
                        &mut Cursor::new(wire),
                        &BTreeMap::from([(header.into(), value.into())]),
                    )
                })
                .is_err()
            );
        }
    }
    #[test]
    fn unframed_http_bodies_still_require_a_clean_eof() {
        let headers = BTreeMap::new();
        assert_eq!(read_body(&mut Cursor::new(b"hello"), &headers), b"hello");
        assert!(
            std::panic::catch_unwind(|| {
                read_body(
                    &mut BufReader::new(ResetAfter(Cursor::new(b"hello".to_vec()))),
                    &headers,
                )
            })
            .is_err()
        );
    }
    #[test]
    fn sse_lines_survive_transport_chunks_and_utf8_boundaries() {
        let source = "data: {\"text\":\"héllo\"}\n\n".as_bytes();
        let mut encoded = Vec::new();
        for part in source.chunks(2) {
            encoded.extend_from_slice(format!("{:x};fixture=yes\r\n", part.len()).as_bytes());
            encoded.extend_from_slice(part);
            encoded.extend_from_slice(b"\r\n");
        }
        encoded.extend_from_slice(b"0\r\n\r\n");
        let mut body = EventBody {
            reader: Cursor::new(encoded),
            chunked: true,
            remaining: 0,
        };
        assert_eq!(body.line().unwrap(), "data: {\"text\":\"héllo\"}");
        assert_eq!(body.line().unwrap(), "");
        assert_eq!(body.line(), None);
    }
}
