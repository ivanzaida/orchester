//! The URL `browser_open` is given: only an `http` or `https` URL opens, and its host decides whether the call asks.
//! Every other kind is refused with an error that names why (`docs/design/browser.md`, "Permissions and policy"):
//! `file:` pages, the browser's own pages (`chrome:`, `edge:`, `about:`), `javascript:` and `data:` URLs, and a URL
//! that carries a user name or password, as no browser tool takes a secret.

use thiserror::Error;

use crate::permission::browser::SiteHost;

/// The longest URL a call may give.
const MAX_URL_CHARS: usize = 8192;

/// Schemes that name no web page of a host; anything else without `//` reads as a host and port (`localhost:5173`).
const OTHER_SCHEMES: &[&str] = &[
  "about",
  "blob",
  "chrome",
  "chrome-extension",
  "data",
  "devtools",
  "edge",
  "file",
  "filesystem",
  "ftp",
  "javascript",
  "mailto",
  "tel",
  "view-source",
  "ws",
  "wss",
];

/// Why a URL is not opened.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum BlockedUrl {
  #[error("`{0}:` URLs are blocked for agents: only http and https pages open")]
  Scheme(String),
  #[error("`{0}` has no scheme: give the whole URL, starting with http:// or https://")]
  NoScheme(String),
  #[error(
    "a URL with a user name or password is refused: no browser tool takes a secret, and sign-ins are the person's, in the Browser window"
  )]
  Credentials,
  #[error("`{0}` is not a URL with a host")]
  NotAUrl(String),
  #[error("a URL is at most {MAX_URL_CHARS} characters long")]
  TooLong,
}

/// An `http` or `https` URL and its host.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PageUrl {
  url: String,
  host: SiteHost,
}

impl PageUrl {
  pub fn parse(text: &str) -> Result<Self, BlockedUrl> {
    let url = text.trim();
    if url.chars().count() > MAX_URL_CHARS {
      return Err(BlockedUrl::TooLong);
    }
    let not_a_url = || BlockedUrl::NotAUrl(url.to_owned());
    let Some((scheme, rest)) = url.split_once(':').filter(|(scheme, _)| is_scheme(scheme)) else {
      return Err(BlockedUrl::NoScheme(url.to_owned()));
    };
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
      return Err(if rest.starts_with("//") || OTHER_SCHEMES.contains(&scheme.as_str()) {
        BlockedUrl::Scheme(scheme)
      } else {
        BlockedUrl::NoScheme(url.to_owned())
      });
    }
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
      return Err(not_a_url());
    }
    let authority = rest
      .strip_prefix("//")
      .ok_or_else(not_a_url)?
      .split(['/', '\\', '?', '#'])
      .next()
      .unwrap_or_default();
    if authority.contains('@') {
      return Err(BlockedUrl::Credentials);
    }
    let host = SiteHost::new(without_port(authority).ok_or_else(not_a_url)?).ok_or_else(not_a_url)?;
    Ok(Self {
      url: url.to_owned(),
      host,
    })
  }

  pub fn as_str(&self) -> &str {
    &self.url
  }

  /// The host whose site decides whether opening it asks.
  pub fn host(&self) -> &SiteHost {
    &self.host
  }
}

/// A URL scheme: a letter, then letters, digits, `+`, `-` and `.`.
fn is_scheme(text: &str) -> bool {
  text.starts_with(|c: char| c.is_ascii_alphabetic())
    && text
      .bytes()
      .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
}

/// `authority` without its port, `None` when the port is not one.
fn without_port(authority: &str) -> Option<&str> {
  let split = match authority.strip_prefix('[') {
    Some(_) => authority.find(']').map(|end| end + 1),
    None => authority.find(':'),
  };
  let Some(end) = split else {
    return Some(authority);
  };
  let (host, port) = authority.split_at(end);
  match port.strip_prefix(':') {
    None if port.is_empty() => Some(host),
    Some(number) if number.is_empty() || number.parse::<u16>().is_ok_and(|port| port > 0) => Some(host),
    _ => None,
  }
}
