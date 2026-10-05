//! What a request must bring before the MCP layer reads it: no `Origin` but a loopback one, so a web
//! page cannot reach the server through the person's browser, and the bearer token of the discovery
//! file.

use axum::{
  extract::{Request, State},
  http::{
    HeaderMap, StatusCode,
    header::{AUTHORIZATION, ORIGIN},
  },
  middleware::Next,
  response::{IntoResponse, Response},
};

use super::server::BearerToken;

/// Hosts an `Origin` may name.
const LOOPBACK_HOSTS: [&str; 3] = ["127.0.0.1", "localhost", "[::1]"];

/// Lets a request through only from no origin or a loopback one, and with the bearer token.
pub async fn admit(State(token): State<BearerToken>, request: Request, next: Next) -> Response {
  if !origin_allowed(request.headers()) {
    return (
      StatusCode::FORBIDDEN,
      "requests from a web origin other than loopback are refused",
    )
      .into_response();
  }
  let presented = request
    .headers()
    .get(AUTHORIZATION)
    .and_then(|value| value.to_str().ok())
    .and_then(|value| value.split_once(' '))
    .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
    .map(|(_, presented)| presented.trim());
  if presented.is_some_and(|presented| token.admits(presented)) {
    return next.run(request).await;
  }
  (
    StatusCode::UNAUTHORIZED,
    "missing or invalid bearer token; read it from Orchester's MCP discovery file",
  )
    .into_response()
}

/// Whether the request has no `Origin`, or an `http`/`https` one on a loopback host.
fn origin_allowed(headers: &HeaderMap) -> bool {
  let Some(origin) = headers.get(ORIGIN) else {
    return true;
  };
  let Some(rest) = origin.to_str().ok().and_then(|origin| {
    origin
      .strip_prefix("http://")
      .or_else(|| origin.strip_prefix("https://"))
  }) else {
    return false;
  };
  let host = match rest.rsplit_once(':') {
    Some((host, port)) if !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()) => host,
    _ => rest,
  };
  LOOPBACK_HOSTS
    .iter()
    .any(|loopback| host.eq_ignore_ascii_case(loopback))
}
