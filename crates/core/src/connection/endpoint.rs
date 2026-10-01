//! Where a connection reaches what it connects to (`docs/architecture/native-agent.md` §2.1, §7): a
//! program on this computer (the agent CLIs and the GitHub CLI), or a model API that Orchester talks
//! to itself (native agents). An API endpoint names its project secret holding the key, never the
//! key itself.

use std::{
  fmt,
  path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::http_url::{self, Scheme};
use crate::id::SecretId;

/// Longest base URL, in characters.
const MAX_URL_CHARS: usize = 2048;
/// Hosts a base URL may reach over plain `http://`: this computer only.
const LOOPBACK_HOSTS: [&str; 3] = ["localhost", "127.0.0.1", "[::1]"];

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum EndpointError {
  #[error("a native connection needs a base URL")]
  BlankUrl,
  #[error("the base URL starts with https://, or with http:// for localhost, 127.0.0.1 or [::1]")]
  UrlScheme,
  #[error("the base URL names no host")]
  NoHost,
  /// A user name or password in the URL would be shown and stored in the clear; the key belongs in
  /// a project secret.
  #[error("the base URL must not contain a user name or password")]
  UrlCredentials,
  #[error("the base URL must not contain a query or a fragment")]
  UrlQuery,
  #[error("the base URL contains a space or a control character")]
  Whitespace,
  #[error("the base URL must not contain a backslash")]
  Backslash,
  /// Characters a host name cannot have, an unclosed IPv6 address, or a port that is not a number.
  #[error("the base URL's host or port is not valid")]
  InvalidHost,
  #[error("the base URL is longer than {MAX_URL_CHARS} characters")]
  TooLong,
}

/// How a connection is reached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentEndpoint {
  /// A program on this computer, by its absolute path.
  Program(PathBuf),
  /// A model API Orchester calls itself.
  Api(ApiEndpoint),
}

impl AgentEndpoint {
  /// The program's path; `None` for an API endpoint.
  pub fn program(&self) -> Option<&Path> {
    match self {
      Self::Program(path) => Some(path),
      Self::Api(_) => None,
    }
  }

  /// The API endpoint; `None` for a program.
  pub const fn api(&self) -> Option<&ApiEndpoint> {
    match self {
      Self::Program(_) => None,
      Self::Api(api) => Some(api),
    }
  }
}

/// Which provider's servers a native connection talks to; it decides the base URL and the
/// provider quirks (§3.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointPreset {
  /// The provider's own API.
  Official,
  /// DeepSeek's OpenAI Responses route; not offered on the Anthropic API (its Anthropic route is
  /// reachable only through [`Self::Custom`]).
  #[serde(rename = "deepseek")]
  DeepSeek,
  /// Another compatible server, treated conservatively.
  Custom,
}

/// A model API: its preset, base URL and the project secret holding its key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiEndpoint {
  preset: EndpointPreset,
  base_url: BaseUrl,
  key: SecretId,
}

impl ApiEndpoint {
  pub const fn new(preset: EndpointPreset, base_url: BaseUrl, key: SecretId) -> Self {
    Self { preset, base_url, key }
  }

  pub const fn preset(&self) -> EndpointPreset {
    self.preset
  }

  pub const fn base_url(&self) -> &BaseUrl {
    &self.base_url
  }

  /// The project secret holding the API key.
  pub const fn key(&self) -> SecretId {
    self.key
  }
}

/// The base URL of a model API: `https://`, or `http://` for this computer only; a host and an
/// optional port, no user name or password, no query or fragment, no whitespace or backslash. Kept
/// without a trailing slash.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BaseUrl(String);

impl BaseUrl {
  /// Surrounding whitespace and trailing slashes are not part of the URL.
  pub fn new(value: &str) -> Result<Self, EndpointError> {
    let url = value.trim();
    if url.is_empty() {
      return Err(EndpointError::BlankUrl);
    }
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
      return Err(EndpointError::Whitespace);
    }
    if url.chars().count() > MAX_URL_CHARS {
      return Err(EndpointError::TooLong);
    }
    // HTTP clients read a backslash as a slash, so it could end the host where this check does not.
    if url.contains('\\') {
      return Err(EndpointError::Backslash);
    }
    if url.contains(['?', '#']) {
      return Err(EndpointError::UrlQuery);
    }
    let (scheme, rest) = http_url::split_scheme(url).ok_or(EndpointError::UrlScheme)?;
    let authority = http_url::authority(rest);
    if authority.contains('@') {
      return Err(EndpointError::UrlCredentials);
    }
    let host = host_of(authority)?;
    let loopback = LOOPBACK_HOSTS.iter().any(|local| local.eq_ignore_ascii_case(host));
    if scheme == Scheme::Http && !loopback {
      return Err(EndpointError::UrlScheme);
    }
    Ok(Self(url.trim_end_matches('/').to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }

  /// The host and port, as the connection list shows where the API is (`api.deepseek.com`).
  pub fn host(&self) -> &str {
    let rest = self.0.split_once("://").map_or(self.0.as_str(), |(_, rest)| rest);
    http_url::authority(rest)
  }
}

/// The host of `authority`, checked with its port: a bracketed IPv6 address or a name of letters,
/// digits, `-`, `.` and `_`, then nothing or `:` and a port number.
fn host_of(authority: &str) -> Result<&str, EndpointError> {
  let (host, port) = match authority.strip_prefix('[') {
    Some(inside) => {
      let end = inside.find(']').ok_or(EndpointError::InvalidHost)?;
      let address = &inside[..end];
      if address.is_empty() || !address.chars().all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.') {
        return Err(EndpointError::InvalidHost);
      }
      let port = match &inside[end + 1..] {
        "" => None,
        after => Some(after.strip_prefix(':').ok_or(EndpointError::InvalidHost)?),
      };
      (&authority[..end + 2], port)
    }
    None => match authority.split_once(':') {
      Some((host, port)) => (host, Some(port)),
      None => (authority, None),
    },
  };
  if host.is_empty() {
    return Err(EndpointError::NoHost);
  }
  let name = |c: char| c.is_alphanumeric() || matches!(c, '-' | '.' | '_');
  if !host.starts_with('[') && !host.chars().all(name) {
    return Err(EndpointError::InvalidHost);
  }
  // `u16` parsing alone would take a sign.
  let valid_port = |port: &str| port.chars().all(|c| c.is_ascii_digit()) && port.parse::<u16>().is_ok();
  if port.is_some_and(|port| !valid_port(port)) {
    return Err(EndpointError::InvalidHost);
  }
  Ok(host)
}

impl fmt::Display for BaseUrl {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for BaseUrl {
  type Error = EndpointError;

  fn try_from(value: String) -> Result<Self, EndpointError> {
    Self::new(&value)
  }
}

impl From<BaseUrl> for String {
  fn from(url: BaseUrl) -> Self {
    url.0
  }
}
