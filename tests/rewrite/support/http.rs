//! Minimal real loopback HTTP/SSE transport for contract integration tests.
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read, Write},
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
    let mut socket = TcpStream::connect(("127.0.0.1", port)).expect("connect server");
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
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).expect("HTTP body");
    let body = if headers
        .get("transfer-encoding")
        .is_some_and(|s| s == "chunked")
    {
        decode_chunks(&bytes)
    } else {
        bytes
    };
    Response {
        status,
        headers,
        body,
    }
}
fn decode_chunks(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    let mut rest = bytes;
    loop {
        let end = rest
            .windows(2)
            .position(|p| p == b"\r\n")
            .expect("chunk length line");
        let size = usize::from_str_radix(
            std::str::from_utf8(&rest[..end])
                .unwrap()
                .split(';')
                .next()
                .unwrap(),
            16,
        )
        .unwrap();
        if size == 0 {
            break;
        }
        rest = &rest[end + 2..];
        output.extend_from_slice(&rest[..size]);
        rest = &rest[size + 2..];
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
        let socket = connect(port, "GET", path, &BTreeMap::new(), &[]);
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
    use super::EventBody;
    use std::io::Cursor;
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
