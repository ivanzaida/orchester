//! Whether an agent's commands and tools may reach the network: not at all, only listed hosts, or any host
//! (`docs/architecture/server.md`, row 75, "What the web editor can pick"). The values are ordered
//! Off < Hosts < On, and a host list is narrower than another when the other covers each of its hosts
//! ([`NetworkAccess::covers`]); moving up that order widens ([`super::widening::Widening::Network`]).

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum HostPatternError {
  #[error("a host must not be blank")]
  Blank,
  #[error("a host is at most {max} characters long", max = HostPattern::MAX_CHARS)]
  TooLong,
  #[error("a host is written without a scheme such as https://")]
  Scheme,
  #[error("a host is written without a path")]
  Path,
  #[error("a host is written without a port")]
  Port,
  #[error("a host must not contain `{0}`")]
  ForbiddenCharacter(char),
  /// An empty part between dots, one longer than 63 characters, or one that starts or ends with `-`.
  #[error("`{0}` is not a part of a host name")]
  Label(String),
}

/// A host agents may reach, as a lowercased name or IPv4 address (`api.example.com`), or `*.` and a domain for
/// every subdomain of it (`*.example.org` covers `api.example.org` and `a.b.example.org`, not `example.org`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct HostPattern(String);

impl HostPattern {
  /// The longest host name DNS allows, without the `*.` of a pattern.
  pub const MAX_CHARS: usize = 253;
  const MAX_LABEL_CHARS: usize = 63;
  const WILDCARD: &'static str = "*.";

  pub fn new(value: &str) -> Result<Self, HostPatternError> {
    let pattern = value.trim().to_ascii_lowercase();
    if pattern.is_empty() {
      return Err(HostPatternError::Blank);
    }
    if pattern.contains("://") {
      return Err(HostPatternError::Scheme);
    }
    if pattern.contains('/') {
      return Err(HostPatternError::Path);
    }
    if pattern.contains(':') {
      return Err(HostPatternError::Port);
    }
    let domain = pattern.strip_prefix(Self::WILDCARD).unwrap_or(&pattern);
    if domain.chars().count() > Self::MAX_CHARS {
      return Err(HostPatternError::TooLong);
    }
    for label in domain.split('.') {
      if let Some(c) = label.chars().find(|c| !(c.is_ascii_alphanumeric() || *c == '-')) {
        return Err(HostPatternError::ForbiddenCharacter(c));
      }
      if label.is_empty() || label.len() > Self::MAX_LABEL_CHARS || label.starts_with('-') || label.ends_with('-') {
        return Err(HostPatternError::Label(label.to_owned()));
      }
    }
    Ok(Self(pattern))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }

  /// The domain whose subdomains a `*.` pattern covers; `None` for a single host.
  fn wildcard_domain(&self) -> Option<&str> {
    self.0.strip_prefix(Self::WILDCARD)
  }

  /// Whether every host `other` lets agents reach, this pattern lets them reach too.
  pub fn covers(&self, other: &Self) -> bool {
    let Some(domain) = self.wildcard_domain() else {
      return self == other;
    };
    let is_subdomain = |host: &str| host.strip_suffix(domain).is_some_and(|rest| rest.ends_with('.'));
    match other.wildcard_domain() {
      Some(other_domain) => other_domain == domain || is_subdomain(other_domain),
      None => is_subdomain(&other.0),
    }
  }
}

impl fmt::Display for HostPattern {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for HostPattern {
  type Error = HostPatternError;

  fn try_from(value: String) -> Result<Self, HostPatternError> {
    Self::new(&value)
  }
}

impl From<HostPattern> for String {
  fn from(pattern: HostPattern) -> Self {
    pattern.0
  }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum NetworkHostsError {
  /// No host at all is network off, which is its own value.
  #[error("at least one host must be listed")]
  Empty,
  #[error("at most {max} hosts can be listed", max = NetworkHosts::MAX_ENTRIES)]
  TooMany,
}

/// The hosts of [`NetworkAccess::Hosts`]: sorted, without duplicates, never empty.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "Vec<HostPattern>", into = "Vec<HostPattern>")]
pub struct NetworkHosts(Vec<HostPattern>);

impl NetworkHosts {
  pub const MAX_ENTRIES: usize = 64;

  pub fn new(mut hosts: Vec<HostPattern>) -> Result<Self, NetworkHostsError> {
    hosts.sort();
    hosts.dedup();
    if hosts.is_empty() {
      return Err(NetworkHostsError::Empty);
    }
    if hosts.len() > Self::MAX_ENTRIES {
      return Err(NetworkHostsError::TooMany);
    }
    Ok(Self(hosts))
  }

  pub fn entries(&self) -> &[HostPattern] {
    &self.0
  }

  /// Whether some pattern of this list covers each pattern of `other`.
  pub fn covers(&self, other: &Self) -> bool {
    other
      .0
      .iter()
      .all(|host| self.0.iter().any(|pattern| pattern.covers(host)))
  }
}

impl TryFrom<Vec<HostPattern>> for NetworkHosts {
  type Error = NetworkHostsError;

  fn try_from(hosts: Vec<HostPattern>) -> Result<Self, NetworkHostsError> {
    Self::new(hosts)
  }
}

impl From<NetworkHosts> for Vec<HostPattern> {
  fn from(hosts: NetworkHosts) -> Self {
    hosts.0
  }
}

/// Whether the agent's commands and tools may reach the network. Stored as `"off"`, `{ "hosts": [...] }` or
/// `"on"`, so bodies saved before the host list existed read as they were.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkAccess {
  Off,
  /// Only these hosts.
  Hosts(NetworkHosts),
  On,
}

impl NetworkAccess {
  /// Whether this access lets agents reach everything `other` does: Off < Hosts < On, and one list covers
  /// another when it covers each of its hosts.
  pub fn covers(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::On, _) | (_, Self::Off) => true,
      (Self::Hosts(hosts), Self::Hosts(others)) => hosts.covers(others),
      (Self::Off, _) | (Self::Hosts(_), Self::On) => false,
    }
  }
}
