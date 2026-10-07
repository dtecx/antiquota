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
