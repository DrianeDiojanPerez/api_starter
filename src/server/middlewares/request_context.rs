use axum::extract::{ConnectInfo, Request};
use axum::middleware::Next;
use axum::response::Response;
use std::net::SocketAddr;
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

    let span = tracing::info_span!(
        "request",
        route_path = %route_path,
        request_id = %request_id,
        ip_address = %ip_address,
        method = %request.method(),
    );

    next.run(request).instrument(span).await
}
