//! The hosts of web pages and the sites a profile allows its agents' browser to open and act on
//! (`docs/design/browser.md`, "Permissions and policy"): a host compares in lower case without a trailing dot, an
//! allowed site is one host (`docs.rs`) or every host under one (`*.example.com`), and the local development hosts are
//! always allowed while the browser is on.

use std::{fmt, net::Ipv6Addr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The longest host name DNS allows.
const MAX_HOST_CHARS: usize = 253;
const MAX_LABEL_CHARS: usize = 63;

/// The host of a web page: DNS names of letters, digits and `-` between dots, or an IPv6 address in brackets; in
/// lower case, without a trailing dot.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SiteHost(String);

impl SiteHost {
  /// `text` as a host, or `None` when it is not one.
  pub fn new(text: &str) -> Option<Self> {
    if let Some(inside) = text.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
      let address: Ipv6Addr = inside.parse().ok()?;
      return Some(Self(format!("[{address}]")));
    }
    let host = text.strip_suffix('.').unwrap_or(text).to_ascii_lowercase();
    let valid = !host.is_empty() && host.len() <= MAX_HOST_CHARS && host.split('.').all(valid_label);
    valid.then_some(Self(host))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }

  /// Whether this host lies under `parent` (`a.b.example.com` under `example.com`), not `parent` itself.
  fn is_under(&self, parent: &Self) -> bool {
    self
      .0
      .strip_suffix(parent.0.as_str())
      .is_some_and(|rest| rest.len() > 1 && rest.ends_with('.'))
  }

  /// Whether this is a local development host, which the browser may open and act on without asking whatever sites
  /// the profile lists: `localhost`, `127.0.0.1`, `[::1]`, and the hosts under `localhost` and `test`.
  pub fn is_local(&self) -> bool {
    matches!(self.0.as_str(), "localhost" | "127.0.0.1" | "[::1]")
      || self.0.ends_with(".localhost")
      || self.0.ends_with(".test")
  }

  fn is_address(&self) -> bool {
    self.0.starts_with('[') || self.0.split('.').all(|label| label.bytes().all(|b| b.is_ascii_digit()))
  }
}

impl fmt::Display for SiteHost {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

/// A DNS label: 1 to 63 letters, digits and `-`, neither first nor last.
fn valid_label(label: &str) -> bool {
  (1..=MAX_LABEL_CHARS).contains(&label.len())
    && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    && !label.starts_with('-')
    && !label.ends_with('-')
}

/// Why an entry of the allowed sites is refused; the profile shows it under the field, which keeps the text.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AllowedSiteError {
  #[error("enter a host such as staging.example.com")]
  Blank,
  #[error("a site is one host, without spaces")]
  Spaces,
  #[error("enter the host without its scheme (https://)")]
  Scheme,
  #[error("enter the host without a path, query or fragment")]
  Path,
  #[error("enter the host without a port: every port of an allowed host is allowed")]
  Port,
  #[error("enter the host without a user name or password")]
  Credentials,
  #[error("a `*` may only start the entry, as in *.example.com")]
  Wildcard,
  #[error("*.{0} would allow too many sites: name a domain under it, as in *.example.{0}")]
  TooBroad(String),
  #[error("enter an international name in its ASCII form (xn--…)")]
  NotAscii,
  #[error("`{0}` is not a host: letters, digits and `-` between dots, or an IPv6 address in brackets")]
  NotAHost(String),
}

/// A site the agent may open and act on without asking: one host, or every host under a domain.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum AllowedSite {
  /// `docs.rs`: this host only.
  Host(SiteHost),
  /// `*.example.com`: every host under `example.com`, not `example.com` itself.
  Under(SiteHost),
}

impl AllowedSite {
  /// `text` as a site, trimmed, or why it is not one: a URL with a scheme, a path or a port, or anything else that is
  /// not a host, is refused with its reason.
  pub fn new(text: &str) -> Result<Self, AllowedSiteError> {
    let text = text.trim();
    if text.is_empty() {
      return Err(AllowedSiteError::Blank);
    }
    refuse_url_parts(text)?;
    if !text.is_ascii() {
      return Err(AllowedSiteError::NotAscii);
    }
    let (under, host) = match text.strip_prefix("*.") {
      Some(host) => (true, host),
      None => (false, text),
    };
    if host.contains('*') {
      return Err(AllowedSiteError::Wildcard);
    }
    let parsed = SiteHost::new(host).ok_or_else(|| AllowedSiteError::NotAHost(host.to_owned()))?;
    if !under {
      return Ok(Self::Host(parsed));
    }
    if parsed.is_address() {
      return Err(AllowedSiteError::Wildcard);
    }
    if !parsed.as_str().contains('.') {
      return Err(AllowedSiteError::TooBroad(parsed.0));
    }
    Ok(Self::Under(parsed))
  }

  /// Whether a page of `host` is this site.
  pub fn matches(&self, host: &SiteHost) -> bool {
    match self {
      Self::Host(site) => site == host,
      Self::Under(domain) => host.is_under(domain),
    }
  }

  /// Whether every host `other` allows, this site allows too.
  pub fn covers(&self, other: &Self) -> bool {
    match (self, other) {
      (_, Self::Host(host)) => self.matches(host),
      (Self::Under(domain), Self::Under(other)) => other == domain || other.is_under(domain),
      (Self::Host(_), Self::Under(_)) => false,
    }
  }
}

/// The parts of a URL that a host has not: whitespace, a scheme, a path, query or fragment, credentials, a port.
fn refuse_url_parts(text: &str) -> Result<(), AllowedSiteError> {
  if text.chars().any(|c| c.is_whitespace() || c.is_control()) {
    return Err(AllowedSiteError::Spaces);
  }
  if text.contains("://") {
    return Err(AllowedSiteError::Scheme);
  }
  if text.contains(['/', '\\', '?', '#']) {
    return Err(AllowedSiteError::Path);
  }
  if text.contains('@') {
    return Err(AllowedSiteError::Credentials);
  }
  let after_address = text.rsplit_once(']').map_or(text, |(_, rest)| rest);
  match after_address.rsplit_once(':') {
    Some((_, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => Err(AllowedSiteError::Port),
    Some(_) if !text.starts_with('[') => Err(AllowedSiteError::Scheme),
    _ => Ok(()),
  }
}

impl fmt::Display for AllowedSite {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Host(host) => write!(f, "{host}"),
      Self::Under(domain) => write!(f, "*.{domain}"),
    }
  }
}

impl TryFrom<String> for AllowedSite {
  type Error = AllowedSiteError;

  fn try_from(value: String) -> Result<Self, AllowedSiteError> {
    Self::new(&value)
  }
}

impl From<AllowedSite> for String {
  fn from(site: AllowedSite) -> Self {
    site.to_string()
  }
}
