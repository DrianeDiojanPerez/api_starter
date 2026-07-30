use std::net::SocketAddr;
use std::time::Instant;

use axum::extract::{ConnectInfo, Request};
use axum::middleware::Next;
use axum::response::Response;
use tracing::Instrument;

pub async fn request_context(request: Request, next: Next) -> Response {
    let route_path = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|path| path.as_str().to_owned())
        .unwrap_or_else(|| "none".to_owned());

    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();

    let ip_address = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip().to_string())
        .unwrap_or_default();

    let method = request.method().clone();
    let uri = request.uri().path().to_owned();

    let span = tracing::info_span!(
        "request",
        route_path = %route_path,
        request_id = %request_id,
        ip_address = %ip_address,
        method = %method,
    );

    async move {
        tracing::info!(method = %method, uri = %uri, "request started");

        let started = Instant::now();
        let response = next.run(request).await;
        let latency = started.elapsed();

        let status = response.status();
        // Sub-millisecond requests are the common case here, so the whole
        // number of milliseconds alone would read as zero for most of them.
        let latency_ms = latency.as_secs_f64() * 1_000.0;

        if status.is_server_error() {
            tracing::error!(
                method = %method,
                uri = %uri,
                status = status.as_u16(),
                latency_ms,
                "request failed"
            );
        } else {
            tracing::info!(
                method = %method,
                uri = %uri,
                status = status.as_u16(),
                latency_ms,
                "request completed"
            );
        }

        response
    }
    .instrument(span)
    .await
}
