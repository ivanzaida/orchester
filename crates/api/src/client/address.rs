//! A server's address as the person gives it: `http(s)://host[:port][/path prefix]`, normalized so
//! one server has one address (`docs/architecture/server.md`, "Desktop data"), and the transport
//! rule of "Security": plain HTTP only for this computer or after the person's opt-in.

use std::{
  fmt,
  net::{Ipv4Addr, Ipv6Addr},
};

use reqwest::Url;
use thiserror::Error;

/// Why a typed address is not a server address.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum AddressError {
  #[error("the address is empty")]
  Empty,
  #[error("the address is not a URL")]
  Malformed,
  #[error("the address is not http:// or https://")]
  Scheme,
  /// User names, passwords, queries and fragments have no place in a server address.
  #[error("the address carries credentials, a query or a fragment")]
  Extra,
}

/// A normalized server address: scheme, host, port when not the default, and a path prefix without
/// a trailing slash.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ServerAddress {
  url: Url,
  text: String,
}

/// Whether the person allowed plain HTTP for a server that is not on this computer, after the
/// warning that the password and token can be read on the network. Stored with the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlainHttp {
  NotAllowed,
  Allowed,
}

impl ServerAddress {
  /// Parses what the person typed. Without a scheme the address is taken as `https://`.
  pub fn parse(text: &str) -> Result<Self, AddressError> {
    let text = text.trim();
    if text.is_empty() {
      return Err(AddressError::Empty);
    }
    let url = if text.contains("://") {
      Url::parse(text)
    } else {
      Url::parse(&format!("https://{text}"))
    }
    .map_err(|_| AddressError::Malformed)?;
    if !matches!(url.scheme(), "http" | "https") {
      return Err(AddressError::Scheme);
    }
    if url.host_str().is_none_or(str::is_empty) {
      return Err(AddressError::Malformed);
    }
    if !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some() {
      return Err(AddressError::Extra);
    }
    let prefix = url.path().trim_end_matches('/');
    let mut text = format!("{}://{}", url.scheme(), authority(&url));
    text.push_str(prefix);
    let url = Url::parse(&text).map_err(|_| AddressError::Malformed)?;
    Ok(Self { url, text })
  }

  /// The normalized address, e.g. `https://orchester.kontur.lan` or `http://localhost:8740/orchester`.
  pub fn as_str(&self) -> &str {
    &self.text
  }

  /// The host and the port when it is not the scheme's default, e.g. `10.0.4.20:8740`.
  pub fn authority(&self) -> String {
    authority(&self.url)
  }

  /// The host alone, e.g. `orchester.kontur.lan`, or `[::1]` for an IPv6 address.
  pub fn host(&self) -> &str {
    self.url.host_str().unwrap_or_default()
  }

  pub fn is_https(&self) -> bool {
    self.url.scheme() == "https"
  }

  /// `localhost`, `127.0.0.0/8` or `[::1]`: plain HTTP to it never leaves this computer.
  pub fn is_loopback(&self) -> bool {
    let host = self.host();
    if let Some(inner) = host.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
      return inner.parse::<Ipv6Addr>() == Ok(Ipv6Addr::LOCALHOST);
    }
    match host.parse::<Ipv4Addr>() {
      Ok(address) => address.is_loopback(),
      Err(_) => host.eq_ignore_ascii_case("localhost"),
    }
  }

  /// Whether talking to the server needs the person's opt-in: plain HTTP to another computer.
  pub fn needs_opt_in(&self) -> bool {
    !self.is_https() && !self.is_loopback()
  }

  /// The URL of `path` under the API's base path, e.g. `/server`.
  pub(crate) fn endpoint(&self, path: &str) -> Result<Url, AddressError> {
    Url::parse(&format!("{}/api/v1{path}", self.text)).map_err(|_| AddressError::Malformed)
  }
}

impl fmt::Debug for ServerAddress {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_tuple("ServerAddress").field(&self.text).finish()
  }
}

impl fmt::Display for ServerAddress {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.text)
  }
}

/// The URL parser already lowercases domains and writes IP addresses canonically.
fn authority(url: &Url) -> String {
  let host = url.host_str().unwrap_or_default();
  match url.port() {
    Some(port) => format!("{host}:{port}"),
    None => host.to_owned(),
  }
}
