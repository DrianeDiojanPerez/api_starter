use axum::http::header;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;

/// Embedded so the production image ships the spec without any extra files.
const SPEC: &str = include_str!("openapi.yaml");

const SCALAR: &str = r##"<!doctype html>
<html>
  <head>
    <title>API Starter Reference</title>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <link rel="icon" href="data:image/svg+xml,
      <svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'>
        <defs>
          <linearGradient id='g' x1='0' y1='0' x2='1' y2='1'>
            <stop offset='0' stop-color='%23fb923c'/>
            <stop offset='1' stop-color='%23ea580c'/>
          </linearGradient>
        </defs>
        <rect width='32' height='32' rx='7' fill='url(%23g)'/>
        <path d='M12 10 6.5 16 12 22' fill='none' stroke='%23fff' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round'/>
        <path d='M20 10 25.5 16 20 22' fill='none' stroke='%23fff' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round'/>
        <line x1='18' y1='8.5' x2='14' y2='23.5' stroke='%23fff' stroke-width='2.4' stroke-linecap='round'/>
      </svg>" />
    <style>
      a[href^="https://www.scalar.com"] {
        display: none !important;
      }
      .scalar-mcp-layer-link {
        display: none !important;
      }
      .agent-button-container {
        display: none !important;
      }
      .open-api-client-button {
        display: none !important;
      }
    </style>
  </head>
  <body>
    <script
      id="api-reference"
      data-url="/openapi.yaml"
      data-configuration='{
        "theme": "default",
        "layout": "modern",
        "title": "API Starter",
        "slug": "api-starter",

        "defaultOpenAllTags": true,
        "defaultOpenFirstTag": true,
        "expandAllModelSections": true,
        "expandAllResponses": true,

        "showSidebar": true,
        "showOperationId": false,
        "showDeveloperTools": "localhost",
        "showToolbar": "localhost",
        "hideModels": false,
        "hideClientButton": true,
        "hideTestRequestButton": false,
        "hideSearch": false,
        "hideDarkModeToggle": false,

        "operationTitleSource": "summary",
        "documentDownloadType": "both",
        "orderSchemaPropertiesBy": "alpha",
        "orderRequiredPropertiesFirst": true,
        "withDefaultFonts": true,

        "persistAuth": false,
        "telemetry": true,
        "isEditable": false,
        "isLoading": false,
        "default": false,
        "_integration": "html",

        "externalUrls": {
          "dashboardUrl": "https://dashboard.scalar.com",
          "registryUrl": "https://registry.scalar.com",
          "proxyUrl": "https://proxy.scalar.com",
          "apiBaseUrl": "https://api.scalar.com"
        }
      }'></script>
    <script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
    <script>
      const hideAskAI = () => {
        for (const button of document.querySelectorAll("button")) {
          if (button.textContent.trim() === "Ask AI") {
            button.style.display = "none";
          }
        }
      };
      new MutationObserver(hideAskAI).observe(document.body, {
        childList: true,
        subtree: true,
      });
      hideAskAI();
    </script>
  </body>
</html>
"##;

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
        assert!(SCALAR.contains(r#"data-url="/openapi.yaml""#));
        assert!(SPEC.starts_with("openapi: 3.1.0"));
    }

    /// The options ride in an HTML attribute, so a stray quote or a trailing
    /// comma is invisible until the page silently falls back to defaults.
    #[test]
    fn the_scalar_configuration_is_valid_json() {
        let opening = SCALAR
            .find("data-configuration='")
            .expect("the configuration attribute should be present")
            + "data-configuration='".len();
        let closing = SCALAR[opening..]
            .find('\'')
            .expect("the attribute should be closed");

        let config: serde_json::Value = serde_json::from_str(&SCALAR[opening..opening + closing])
            .expect("the configuration should be valid json");

        assert_eq!(config["slug"], "api-starter");
        assert_eq!(config["layout"], "modern");
        assert!(config["defaultOpenAllTags"].as_bool().unwrap());
    }
}
