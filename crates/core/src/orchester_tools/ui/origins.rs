//! Where the `browser` UI driver's tab may go (`docs/architecture/agent-testing.md`, "UI drivers"): only the origins
//! of the services the step started, so that a step without network access reaches nothing else through its page.
//! Every other navigation and request is blocked and reported. A URL that names no server — `data:`, `blob:` and
//! `about:` — is the page's own content and is let through; every other scheme (`file:`, `chrome:`, `ftp:`, …) is
//! blocked.

use std::{collections::BTreeSet, fmt};

/// Schemes whose URLs carry the page's own content instead of naming a server.
const OWN_CONTENT: [&str; 3] = ["data", "blob", "about"];

/// A web origin as a browser compares them: scheme, host and port, in lower case, the port always written. A
/// WebSocket's origin is that of the page scheme it pairs with (`ws` is `http`, `wss` is `https`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Origin(String);

impl Origin {
  /// The origin of `url`; `None` for a URL that is not `http`, `https`, `ws` or `wss` with a host.
  pub fn of(url: &str) -> Option<Self> {
    let (scheme, rest) = url.trim().split_once("://")?;
    let (scheme, default_port) = match scheme.to_ascii_lowercase().as_str() {
      "http" | "ws" => ("http", 80),
      "https" | "wss" => ("https", 443),
      _ => return None,
    };
    let authority = rest.split(['/', '\\', '?', '#']).next().unwrap_or_default();
    let authority = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    let (host, port) = split_port(authority)?;
    if host.is_empty() {
      return None;
    }
    let port = port.unwrap_or(default_port);
    Some(Self(format!("{scheme}://{}:{port}", host.to_ascii_lowercase())))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for Origin {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

/// `authority`'s host and its port when it writes one; `None` when the port is not a number.
fn split_port(authority: &str) -> Option<(&str, Option<u16>)> {
  let split = match authority.strip_prefix('[') {
    Some(_) => authority.find(']').map(|end| end + 1),
    None => authority.find(':'),
  };
  let Some(end) = split else {
    return Some((authority, None));
  };
  let (host, port) = authority.split_at(end);
  match port.strip_prefix(':') {
    None if port.is_empty() => Some((host, None)),
    Some("") => Some((host, None)),
    Some(number) => number.parse::<u16>().ok().map(|port| (host, Some(port))),
    None => None,
  }
}

/// The origins the tab may open: those of the services the step started.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AllowedOrigins(BTreeSet<Origin>);

impl AllowedOrigins {
  /// The origins of the services listening at `urls`; a URL without one adds nothing.
  pub fn of_services<'a>(urls: impl IntoIterator<Item = &'a str>) -> Self {
    Self(urls.into_iter().filter_map(Origin::of).collect())
  }

  /// Whether the tab may load `url`: one of the services' origins, or the page's own content.
  pub fn allows(&self, url: &str) -> bool {
    if let Some(origin) = Origin::of(url) {
      return self.0.contains(&origin);
    }
    url
      .split_once(':')
      .is_some_and(|(scheme, _)| OWN_CONTENT.contains(&scheme.to_ascii_lowercase().as_str()))
  }

  /// The origins, in order.
  pub fn iter(&self) -> impl Iterator<Item = &Origin> {
    self.0.iter()
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }
}
