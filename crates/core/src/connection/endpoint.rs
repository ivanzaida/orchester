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

/// The base URL of a model API: `https://`, or `http://` for this computer only; a host, no user
/// name or password, no query or fragment, no whitespace. Kept without a trailing slash.
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
    if url.contains(['?', '#']) {
      return Err(EndpointError::UrlQuery);
    }
    let lower = url.to_ascii_lowercase();
    let (secure, rest) = if let Some(rest) = lower.strip_prefix("https://") {
      (true, rest)
    } else if let Some(rest) = lower.strip_prefix("http://") {
      (false, rest)
    } else {
      return Err(EndpointError::UrlScheme);
    };
    let authority = rest.split('/').next().unwrap_or_default();
    if authority.contains('@') {
      return Err(EndpointError::UrlCredentials);
    }
    let host = host_of(authority);
    if host.is_empty() {
      return Err(EndpointError::NoHost);
    }
    if !secure && !LOOPBACK_HOSTS.contains(&host) {
      return Err(EndpointError::UrlScheme);
    }
    Ok(Self(url.trim_end_matches('/').to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// The host of `authority` without its port; an IPv6 host keeps its brackets.
fn host_of(authority: &str) -> &str {
  if authority.starts_with('[') {
    return authority.find(']').map_or(authority, |end| &authority[..=end]);
  }
  authority.split(':').next().unwrap_or_default()
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
