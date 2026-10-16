//! The server stays the front door (`docs/architecture/server.md`, "The server admin", *How it is served*): it answers
//! `/api/*` itself and forwards every other path to the server admin app at `admin_url`, so a server has one address,
//! one port and one certificate. `/` goes to `/admin`. The admin gets the client's address, the scheme and the host
//! the browser used in `X-Forwarded-*` headers the server sets here, replacing any the client sent; the admin listens
//! only on a private address, so it can trust them.

use std::time::Duration;

use axum::{
  body::Body,
  extract::{ConnectInfo, Request, State},
  http::{HeaderMap, HeaderName, HeaderValue, Uri, header},
  response::{IntoResponse, Redirect, Response},
};
use hyper_util::rt::TokioIo;
use orchester_api::ErrorCode;
use tokio::net::TcpStream;

use super::address;
use crate::{Server, error::ServerError, tls::PeerAddr};

/// How long the admin app may take to accept a connection, and to start its answer.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const ANSWER_TIMEOUT: Duration = Duration::from_secs(30);

/// Headers that describe one connection, not the request: never forwarded either way (RFC 9110 §7.6.1).
const HOP_BY_HOP: [&str; 8] = [
  "connection",
  "keep-alive",
  "proxy-authenticate",
  "proxy-authorization",
  "te",
  "trailer",
  "transfer-encoding",
  "upgrade",
];

/// What the client may not tell the admin itself: the server sets the forwarded headers, and the rest describe a
/// proxy's view the admin must not take from a browser. `x-middleware-subrequest` would let a request skip Next.js'
/// own middleware (`proxy.ts`, with its CSP and HTTPS rule).
const FORWARDED: [&str; 10] = [
  "forwarded",
  "x-forwarded-for",
  "x-forwarded-host",
  "x-forwarded-proto",
  "x-forwarded-port",
  "x-forwarded-prefix",
  "x-forwarded-ssl",
  "x-forwarded-scheme",
  "x-real-ip",
  "x-middleware-subrequest",
];

pub(super) async fn forward(State(server): State<Server>, peer: ConnectInfo<PeerAddr>, request: Request) -> Response {
  let path = request.uri().path();
  if path == "/api" || path.starts_with("/api/") {
    return ServerError::from(ErrorCode::NotFound).into_response();
  }
  if path == "/" {
    return Redirect::to("/admin").into_response();
  }
  let scheme = super::scheme::of(&server, peer.0.0.ip(), request.headers());
  let client = address(&server, peer, request.headers());
  match send(&server.inner.config.admin_url, request, &client.to_string(), scheme).await {
    Ok(response) => response,
    Err(Unreachable) => super::admin_unavailable::not_running(),
  }
}

/// The admin app did not answer: it is not running, or not where `admin_url` says.
struct Unreachable;

async fn send(admin_url: &str, request: Request, client: &str, scheme: &str) -> Result<Response, Unreachable> {
  let admin_url: Uri = admin_url.parse().map_err(|_| Unreachable)?;
  let authority = admin_url.authority().ok_or(Unreachable)?.clone();
  let port = authority.port_u16().unwrap_or(80);
  let stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect((authority.host(), port)))
    .await
    .map_err(|_| Unreachable)?
    .map_err(|_| Unreachable)?;
  let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
    .await
    .map_err(|_| Unreachable)?;
  tokio::spawn(async move {
    // The connection ends with the answer; a failure here is the admin closing early, already seen by `send`.
    if connection.await.is_err() {
      tracing::warn!(error_kind = "AdminProxy", "admin app connection failed");
    }
  });
  let (mut parts, body) = request.into_parts();
  let host = parts.headers.get(header::HOST).cloned();
  strip(&mut parts.headers, &FORWARDED);
  strip_hop_by_hop(&mut parts.headers);
  set(&mut parts.headers, "x-forwarded-for", client);
  set(&mut parts.headers, "x-forwarded-proto", scheme);
  if let Some(host) = host {
    parts.headers.insert(HeaderName::from_static("x-forwarded-host"), host);
  }
  parts.uri = parts
    .uri
    .path_and_query()
    .map_or_else(|| Uri::from_static("/"), |path| Uri::from(path.clone()));
  let answer = tokio::time::timeout(ANSWER_TIMEOUT, sender.send_request(Request::from_parts(parts, body)))
    .await
    .map_err(|_| Unreachable)?
    .map_err(|_| Unreachable)?;
  let (mut parts, body) = answer.into_parts();
  strip_hop_by_hop(&mut parts.headers);
  Ok(Response::from_parts(parts, Body::new(body)))
}

/// Removes the hop-by-hop headers, and the ones `Connection` names as this connection's own (RFC 9110 §7.6.1).
fn strip_hop_by_hop(headers: &mut HeaderMap) {
  let named: Vec<HeaderName> = headers
    .get_all(header::CONNECTION)
    .iter()
    .filter_map(|value| value.to_str().ok())
    .flat_map(|value| value.split(','))
    .filter_map(|name| HeaderName::from_bytes(name.trim().as_bytes()).ok())
    .collect();
  for name in named {
    headers.remove(name);
  }
  strip(headers, &HOP_BY_HOP);
}

fn strip(headers: &mut HeaderMap, names: &[&str]) {
  for name in names {
    headers.remove(*name);
  }
}

fn set(headers: &mut HeaderMap, name: &'static str, value: &str) {
  if let Ok(value) = HeaderValue::from_str(value) {
    headers.insert(HeaderName::from_static(name), value);
  }
}
