use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::url::{split_remote, strip_userinfo};

/// Scheme-independent identity of a network git repository; never contains userinfo.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RepositoryIdentity(String);

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum RepositoryIdentityError {
  #[error("the repository has no origin")]
  MissingOrigin,
  #[error("origin must name a network repository")]
  LocalOrigin,
  #[error("origin is not a valid network repository URL")]
  Invalid,
}

impl RepositoryIdentity {
  pub fn from_remote_url(url: &str) -> Result<Self, RepositoryIdentityError> {
    if url.trim().is_empty() {
      return Err(RepositoryIdentityError::MissingOrigin);
    }
    let clean = strip_userinfo(url);
    let (authority, path) = split_remote(&clean).ok_or(RepositoryIdentityError::LocalOrigin)?;
    let host = if let Some(rest) = authority.strip_prefix('[') {
      let (address, suffix) = rest.split_once(']').ok_or(RepositoryIdentityError::Invalid)?;
      address
        .parse::<std::net::Ipv6Addr>()
        .map_err(|_| RepositoryIdentityError::Invalid)?;
      if !suffix.is_empty() && !valid_port(suffix.strip_prefix(':')) {
        return Err(RepositoryIdentityError::Invalid);
      }
      format!("[{address}]")
    } else {
      let (host, port) = authority
        .split_once(':')
        .map_or((authority, None), |(host, port)| (host, Some(port)));
      if port.is_some() && !valid_port(port) {
        return Err(RepositoryIdentityError::Invalid);
      }
      host.to_owned()
    };
    Self::canonical(&format!("{host}/{}", path.trim_matches('/')))
  }

  /// Parses the API's canonical identity, or normalizes a git fetch URL.
  pub fn parse(value: &str) -> Result<Self, RepositoryIdentityError> {
    if value.contains("://") || value.contains('@') || value.contains(':') && !value.starts_with('[') {
      return Self::from_remote_url(value);
    }
    Self::canonical(value)
  }

  fn canonical(value: &str) -> Result<Self, RepositoryIdentityError> {
    let value = value.trim().trim_end_matches('/').to_lowercase();
    let value = value.strip_suffix(".git").unwrap_or(&value);
    let (host, path) = value.split_once('/').ok_or(RepositoryIdentityError::Invalid)?;
    if host.is_empty()
      || path.is_empty()
      || value.contains(['@', '?', '#', '\\'])
      || value.chars().any(char::is_whitespace)
    {
      return Err(RepositoryIdentityError::Invalid);
    }
    if host.starts_with('[') {
      host
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .ok_or(RepositoryIdentityError::Invalid)?
        .parse::<std::net::Ipv6Addr>()
        .map_err(|_| RepositoryIdentityError::Invalid)?;
    } else if !host.bytes().all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b)) || host == "." || host == ".." {
      return Err(RepositoryIdentityError::Invalid);
    }
    if path
      .split('/')
      .any(|p| p.is_empty() || matches!(p, "." | "..") || p.contains(':'))
    {
      return Err(RepositoryIdentityError::Invalid);
    }
    Ok(Self(value.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

fn valid_port(port: Option<&str>) -> bool {
  port.is_some_and(|port| port.parse::<u16>().is_ok_and(|port| port > 0))
}

impl TryFrom<String> for RepositoryIdentity {
  type Error = RepositoryIdentityError;
  fn try_from(value: String) -> Result<Self, Self::Error> {
    Self::parse(&value)
  }
}

impl From<RepositoryIdentity> for String {
  fn from(value: RepositoryIdentity) -> Self {
    value.0
  }
}
