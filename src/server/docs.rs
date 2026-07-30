use axum::http::header;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;

/// Embedded so the production image ships the spec without any extra files.
const SPEC: &str = include_str!("openapi.yaml");

const SCALAR: &str = r#"<!doctype html>
<html>
  <head>
    <title>API Starter reference</title>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
  </head>
  <body>
    <div id="app"></div>
    <script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
    <script>
      Scalar.createApiReference('#app', { url: '/openapi.yaml' })
    </script>
  </body>
</html>
"#;

pub fn routes() -> Router {
    Router::new()
        .route("/docs", get(reference))
        .route("/openapi.yaml", get(spec))
}

async fn reference() -> Html<&'static str> {
    Html(SCALAR)
}

async fn spec() -> Response {
    ([(header::CONTENT_TYPE, "application/yaml")], SPEC).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_served_route_is_documented() {
        for route in [
            "/v1/healthcheck",
            "/v1/login",
            "/v1/refresh-token",
            "/v1/forgot-password",
            "/v1/reset-password",
            "/v1/users",
            "/v1/users/my-user",
            "/v1/users/{user-id}",
            "/v1/permissions",
        ] {
            assert!(
                SPEC.contains(&format!("\n  {route}:\n")),
                "{route} is served but missing from openapi.yaml"
            );
        }
    }

    #[test]
    fn the_reference_page_points_at_the_spec_this_binary_serves() {
        assert!(SCALAR.contains("url: '/openapi.yaml'"));
        assert!(SPEC.starts_with("openapi: 3.1.0"));
    }
}
