//! What a request must bring before the MCP layer reads it: a `Host` that names the loopback address and the
//! server's port, so a web page cannot reach the server through DNS rebinding (a fixed port makes that easier to
//! aim; the MCP specification asks for the check); no `Origin` but a loopback one, so a web page cannot reach it
//! through the person's browser; and the bearer token.

use std::num::NonZeroU16;

use axum::{
  extract::{Request, State},
  http::{
    HeaderMap, StatusCode,
    header::{AUTHORIZATION, HOST, ORIGIN},
  },
  middleware::Next,
  response::{IntoResponse, Response},
};

use super::token::BearerToken;

/// Hosts an `Origin` may name.
const LOOPBACK_HOSTS: [&str; 3] = ["127.0.0.1", "localhost", "[::1]"];
/// Hosts a `Host` header may name: the server listens on IPv4 loopback only.
const SERVED_HOSTS: [&str; 2] = ["127.0.0.1", "localhost"];

/// What a request is admitted with: the token, and the port the server listens on.
#[derive(Clone)]
pub struct Admission {
  token: BearerToken,
  port: NonZeroU16,
}

impl Admission {
  pub const fn new(token: BearerToken, port: NonZeroU16) -> Self {
    Self { token, port }
  }
}

/// Lets a request through only with a loopback `Host` on the server's port, from no origin or a loopback one,
/// and with the bearer token.
pub async fn admit(State(admission): State<Admission>, request: Request, next: Next) -> Response {
  if !host_allowed(request.headers(), admission.port) {
    return (
      StatusCode::FORBIDDEN,
      "the Host header must name 127.0.0.1 or localhost and the server's port",
    )
      .into_response();
  }
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
  if presented.is_some_and(|presented| admission.token.admits(presented)) {
    return next.run(request).await;
  }
  (
    StatusCode::UNAUTHORIZED,
    "missing or invalid bearer token; Orchester's Settings › Automation gives the command that registers it",
  )
    .into_response()
}

/// Whether the request's `Host` is `127.0.0.1:<port>` or `localhost:<port>`. A missing one is refused: HTTP/1.1
/// requires it, and every MCP client sends it.
fn host_allowed(headers: &HeaderMap, port: NonZeroU16) -> bool {
  let Some((host, named)) = headers
    .get(HOST)
    .and_then(|host| host.to_str().ok())
    .and_then(|host| host.rsplit_once(':'))
  else {
    return false;
  };
  named.bytes().all(|byte| byte.is_ascii_digit())
    && named.parse::<u16>().is_ok_and(|named| named == port.get())
    && SERVED_HOSTS.iter().any(|served| host.eq_ignore_ascii_case(served))
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
