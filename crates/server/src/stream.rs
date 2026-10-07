//! Owns SSE transport for the same typed events consumed by the CLI.
use crate::runtime::HttpState;
use axum::{
    http::header,
    response::{
        IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
};
use futures_util::{StreamExt, stream};
use std::{convert::Infallible, time::Duration};
pub fn events(state: &HttpState) -> Response {
    let receiver = state.service.subscribe();
    let closing = state.closing.subscribe();
    let stream = stream::unfold(
        (receiver, closing),
        |(mut receiver, mut closing)| async move {
            if *closing.borrow() {
                return None;
            }
            let record = tokio::select! {result=receiver.recv()=>result.ok()?,_=closing.changed()=>return None};
            let event = Event::default().json_data(record).ok()?;
            Some((Ok::<_, Infallible>(event), (receiver, closing)))
        },
    );
    let stream = stream::once(async { Ok::<_, Infallible>(Event::default().comment("connected")) })
        .chain(stream);
    let mut response = Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("ping"),
        )
        .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    response
}

/// Standard base64 for terminal bytes in JSON frames.
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(TABLE[((n >> shift) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The terminal stream: recent output with shell details, then output chunks (bursts of up to
/// 64 combined), a ping every 15 seconds while quiet, and `{"exit": true}` when the shell ends.
pub fn terminal(
    info: antiquota_core::TerminalInfo,
    feed: antiquota_core::TerminalFeed,
) -> Response {
    use antiquota_core::TerminalChunk;
    let (tx, rx) = tokio::sync::mpsc::channel::<Event>(64);
    let first = serde_json::json!({"d": base64(&feed.recent), "shell": info.shell, "cwd": info.cwd, "alive": info.alive});
    tokio::task::spawn_blocking(move || {
        let send = |event: Event| tx.blocking_send(event).is_ok();
        if !send(Event::default().data(first.to_string())) {
            return;
        }
        loop {
            match feed.chunks.recv_timeout(Duration::from_secs(15)) {
                Ok(TerminalChunk::Data(mut bytes)) => {
                    let mut exit = false;
                    for _ in 1..64 {
                        match feed.chunks.try_recv() {
                            Ok(TerminalChunk::Data(more)) => bytes.extend_from_slice(&more),
                            Ok(TerminalChunk::Exit) => {
                                exit = true;
                                break;
                            }
                            Err(_) => break,
                        }
                    }
                    let frame = serde_json::json!({"d": base64(&bytes)}).to_string();
                    if !send(Event::default().data(frame)) {
                        return;
                    }
                    if exit {
                        let _ = send(Event::default().data(r#"{"exit": true}"#));
                        return;
                    }
                }
                Ok(TerminalChunk::Exit) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    let _ = send(Event::default().data(r#"{"exit": true}"#));
                    return;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if !send(Event::default().comment("ping")) {
                        return;
                    }
                }
            }
        }
    });
    let stream = stream::unfold(rx, |mut rx| async move {
        rx.recv()
            .await
            .map(|event| (Ok::<_, Infallible>(event), rx))
    });
    let mut response = Sse::new(stream).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_matches_the_standard_alphabet() {
        for (input, output) in [
            (&b""[..], ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (b"foobar", "Zm9vYmFy"),
            (&[0xff, 0xfe, 0x00][..], "//4A"),
        ] {
            assert_eq!(super::base64(input), output);
        }
    }
}
