//! The private listener only the server admin's own server side reaches (`admin_api_listen`): it calls the API for a
//! browser, and says which client and which host that browser used with the headers the server's proxy gave it
//! (`admin_proxy`). Here they stand in for the connection's peer, `Host` and [`Scheme`], so the per-address limits
//! count the browser and project addresses carry the host and scheme it used.

use std::net::{IpAddr, SocketAddr};

use axum::{
  extract::{ConnectInfo, Request, State},
  http::{HeaderValue, header::HOST},
  middleware::Next,
  response::Response,
};

use super::scheme::{self, Scheme};
use crate::{Server, tls::PeerAddr};

pub(super) async fn forwarded(State(server): State<Server>, mut request: Request, next: Next) -> Response {
  let client = request
    .headers()
    .get("x-forwarded-for")
    .and_then(|value| value.to_str().ok())
    .and_then(|value| value.rsplit(',').next())
    .and_then(|value| value.trim().parse::<IpAddr>().ok());
  if let Some(client) = client {
    request
      .extensions_mut()
      .insert(ConnectInfo(PeerAddr(SocketAddr::new(client.to_canonical(), 0))));
  }
  let host = request
    .headers()
    .get("x-forwarded-host")
    .filter(|value| value.to_str().is_ok_and(valid_host))
    .cloned();
  let said = request
    .headers()
    .get("x-forwarded-proto")
    .and_then(|value| value.to_str().ok());
  let proto = match said {
    Some("https") => "https",
    Some("http") => "http",
    _ => scheme::configured(&server),
  };
  request.extensions_mut().insert(Scheme(proto));
  let headers = request.headers_mut();
  // Taken in above; the API's own reading of these (trusted proxies) must not see them a second time.
  headers.remove("x-forwarded-for");
  headers.remove("x-forwarded-host");
  headers.remove("x-forwarded-proto");
  if let Some(host) = host {
    headers.insert(HOST, host);
  }
  next.run(request).await
}

fn valid_host(host: &str) -> bool {
  !host.is_empty()
    && host.len() <= 255
    && !host.contains(['/', '?', '#', '@', ','])
    && HeaderValue::from_str(host).is_ok()
    && !host.contains(char::is_whitespace)
}
