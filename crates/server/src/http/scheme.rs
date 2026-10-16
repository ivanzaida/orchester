//! The scheme a request came in by, settled once per request and kept in its extensions ([`Scheme`]), so the admin
//! proxy and project addresses agree on it (security review 1, M1). The public listener decides it from its own
//! certificate, a trusted proxy's word or the configured address; the private admin API listener takes what the admin
//! app passes on, which the server's own proxy set (`internal`).

use std::net::IpAddr;

use axum::{
  extract::{ConnectInfo, Request, State},
  http::HeaderMap,
  middleware::Next,
  response::Response,
};

use crate::{Server, tls::PeerAddr};

#[derive(Clone, Copy, Debug)]
pub(super) struct Scheme(pub &'static str);

/// The scheme the browser or app used: `https` when the server presents its own certificate. Behind a proxy that holds
/// the certificate (`tls = "off"`), what a trusted proxy says in `X-Forwarded-Proto` (one value, `http` or `https`),
/// else [`configured`]. A client that is not a trusted proxy cannot say it.
pub(super) fn of(server: &Server, peer: IpAddr, headers: &HeaderMap) -> &'static str {
  if server.inner.tls.is_some() {
    return "https";
  }
  let trusted = server
    .inner
    .config
    .trusted_proxies
    .iter()
    .any(|proxy| proxy.contains(peer));
  let mut said = headers.get_all("x-forwarded-proto").iter();
  let forwarded = match (said.next(), said.next()) {
    (Some(value), None) if trusted => value.to_str().ok().map(str::trim),
    _ => None,
  };
  match forwarded {
    Some(value) if value.eq_ignore_ascii_case("https") => "https",
    Some(value) if value.eq_ignore_ascii_case("http") => "http",
    _ => configured(server),
  }
}

/// Without a word from a proxy: `https` with the server's own certificate or an `https://` address, else `http`.
pub(super) fn configured(server: &Server) -> &'static str {
  let https = server.inner.tls.is_some()
    || server
      .inner
      .config
      .address
      .as_deref()
      .is_some_and(|address| address.starts_with("https://"));
  if https { "https" } else { "http" }
}

/// The public listener's [`Scheme`] for each request.
pub(super) async fn public(
  State(server): State<Server>,
  ConnectInfo(PeerAddr(peer)): ConnectInfo<PeerAddr>,
  mut request: Request,
  next: Next,
) -> Response {
  let scheme = of(&server, peer.ip(), request.headers());
  request.extensions_mut().insert(Scheme(scheme));
  next.run(request).await
}

/// The server's base address: the configured `address`, or the scheme the request came in by and the `Host` it was
/// sent to. A project's address is this and its slug.
pub(super) fn base_address(server: &Server, scheme: Scheme, headers: &HeaderMap) -> String {
  if let Some(address) = &server.inner.config.address {
    return address.clone();
  }
  let host = headers
    .get(axum::http::header::HOST)
    .and_then(|value| value.to_str().ok())
    .filter(|host| !host.is_empty() && !host.contains(['/', '?', '#', '@']) && !host.contains(char::is_whitespace))
    .unwrap_or("localhost");
  format!("{}://{host}", scheme.0)
}
