//! The server stays the front door (`docs/architecture/server.md`, "The server admin", *How it is served*): it answers
//! `/api/*` itself and forwards every other path to the server admin app at `admin_url`, so a server has one address,
//! one port and one certificate. `/` goes to `/admin`. The admin gets the client's address, the scheme and the host
//! the browser used in `X-Forwarded-*` headers the server sets here, replacing any the client sent; the admin listens
//! only on a private address, so it can trust them.

use std::{net::IpAddr, time::Duration};

use axum::{
  body::Body,
  extract::{ConnectInfo, Request, State},
  http::{HeaderMap, HeaderName, HeaderValue, StatusCode, Uri, header},
  response::{Html, IntoResponse, Redirect, Response},
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

/// What the client may not tell the admin itself: the server sets the forwarded headers.
const FORWARDED: [&str; 4] = ["forwarded", "x-forwarded-for", "x-forwarded-host", "x-forwarded-proto"];

pub(super) async fn forward(State(server): State<Server>, peer: ConnectInfo<PeerAddr>, request: Request) -> Response {
  let path = request.uri().path();
  if path == "/api" || path.starts_with("/api/") {
    return ServerError::from(ErrorCode::NotFound).into_response();
  }
  if path == "/" {
    return Redirect::to("/admin").into_response();
  }
  let scheme = scheme(&server, peer.0.0.ip(), request.headers());
  let client = address(&server, peer, request.headers());
  match send(&server.inner.config.admin_url, request, &client.to_string(), scheme).await {
    Ok(response) => response,
    Err(Unreachable) => not_running(),
  }
}

/// The scheme the browser used: `https` when the server presents its own certificate. Behind a proxy that holds the
/// certificate (`tls = "off"`), what a trusted proxy says in `X-Forwarded-Proto` (one value, `http` or `https`), else
/// the scheme of the configured `address`, else `http`. A client that is not a trusted proxy cannot say it.
fn scheme(server: &Server, peer: IpAddr, headers: &HeaderMap) -> &'static str {
  if server.inner.tls.is_some() {
    return "https";
  }
  let config = &server.inner.config;
  let trusted = config.trusted_proxies.iter().any(|proxy| proxy.contains(peer));
  let mut said = headers.get_all("x-forwarded-proto").iter();
  let forwarded = match (said.next(), said.next()) {
    (Some(value), None) if trusted => value.to_str().ok().map(str::trim),
    _ => None,
  };
  match forwarded {
    Some(value) if value.eq_ignore_ascii_case("https") => "https",
    Some(value) if value.eq_ignore_ascii_case("http") => "http",
    _ if config
      .address
      .as_deref()
      .is_some_and(|address| address.starts_with("https://")) =>
    {
      "https"
    }
    _ => "http",
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
  strip(&mut parts.headers, &HOP_BY_HOP);
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
  strip(&mut parts.headers, &HOP_BY_HOP);
  Ok(Response::from_parts(parts, Body::new(body)))
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

/// j8WujU, *The admin app is not running*. The server has no words of its own beyond its console's, which are
/// English; this page is the one answer it gives a browser without the admin.
fn not_running() -> Response {
  let mut response = (StatusCode::BAD_GATEWAY, Html(include_str!("admin_unavailable.html"))).into_response();
  response.headers_mut().insert(
    header::CONTENT_SECURITY_POLICY,
    HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'; frame-ancestors 'none'"),
  );
  response
}
