//! A decision model's connection (`docs/design/decision-reviewer.md`): a server of the System One API that
//! answers the decision reviewer's questions. It is not an agent connection: it has an id of its own
//! ([`DecisionConnectionId`]), so no agent profile can choose it. Like a native connection it names the project
//! secret holding its key, never the key itself.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
  endpoint::{self, EndpointError, MAX_URL_CHARS},
  http_url::{self, ZoneId},
};
use crate::{
  id::{DecisionConnectionId, SecretId},
  name::Name,
  resource::ModelId,
};

/// The path the System One API is served under; Orchester adds it to the base URL itself.
const API_PATH_SEGMENT: &str = "systemone";
/// The hosts of a server on this computer, without brackets. Any other spelling of the loopback address, `0.0.0.0`
/// and a LAN address are other hosts.
const THIS_COMPUTER_HOSTS: [&str; 3] = ["127.0.0.1", "::1", "localhost"];

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DecisionServerUrlError {
  #[error("a decision model needs its server's base URL")]
  Blank,
  #[error("the base URL starts with http:// or https://")]
  Scheme,
  #[error("the base URL names no host")]
  NoHost,
  /// A user name or password in the URL would be shown and stored in the clear; the key belongs in a project secret.
  #[error("the base URL must not contain a user name or password")]
  Credentials,
  #[error("the base URL must not contain a query or a fragment")]
  Query,
  #[error("the base URL contains a space or a control character")]
  Whitespace,
  #[error("the base URL must not contain a backslash")]
  Backslash,
  /// Characters a host name cannot have, an unclosed IPv6 address, or a port that is not a number.
  #[error("the base URL's host or port is not valid")]
  InvalidHost,
  #[error("the base URL is longer than {MAX_URL_CHARS} characters")]
  TooLong,
  #[error("the base URL ends before /systemone, which Orchester adds itself")]
  ApiPath,
}

/// The base URL of a System One server: `http://` or `https://`, a host and an optional port and path, no user
/// name or password, no query or fragment, no whitespace or backslash, and nothing from `/systemone` on. Kept
/// without a trailing slash.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DecisionServerUrl(String);

impl DecisionServerUrl {
  /// Surrounding whitespace and trailing slashes are not part of the URL.
  pub fn new(value: &str) -> Result<Self, DecisionServerUrlError> {
    let url = value.trim();
    if url.is_empty() {
      return Err(DecisionServerUrlError::Blank);
    }
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
      return Err(DecisionServerUrlError::Whitespace);
    }
    if url.chars().count() > MAX_URL_CHARS {
      return Err(DecisionServerUrlError::TooLong);
    }
    if url.contains(http_url::BACKSLASH) {
      return Err(DecisionServerUrlError::Backslash);
    }
    if url.contains(['?', '#']) {
      return Err(DecisionServerUrlError::Query);
    }
    let (_, rest) = http_url::split_scheme(url).ok_or(DecisionServerUrlError::Scheme)?;
    let authority = http_url::authority(rest);
    if authority.contains('@') {
      return Err(DecisionServerUrlError::Credentials);
    }
    endpoint::host_of(authority).map_err(|error| match error {
      EndpointError::NoHost => DecisionServerUrlError::NoHost,
      _ => DecisionServerUrlError::InvalidHost,
    })?;
    let path = &rest[authority.len()..];
    if path
      .split('/')
      .any(|segment| segment.eq_ignore_ascii_case(API_PATH_SEGMENT))
    {
      return Err(DecisionServerUrlError::ApiPath);
    }
    Ok(Self(url.trim_end_matches('/').to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }

  /// The host without its port; an IPv6 address keeps its brackets (`[::1]`).
  pub fn host(&self) -> &str {
    let rest = self.0.split_once("://").map_or(self.0.as_str(), |(_, rest)| rest);
    let authority = http_url::authority(rest);
    http_url::split_host(authority, ZoneId::Refused).map_or(authority, |(host, _)| host)
  }

  /// Whether the server runs on this computer ([`is_this_computer`]): what decides later that saving or choosing
  /// it is a person's alone.
  pub fn is_on_this_computer(&self) -> bool {
    is_this_computer(self.host())
  }
}

/// Whether `host` names this computer: `127.0.0.1`, `::1` (bracketed or not) or `localhost`, in any case. Nothing
/// else does, not `0.0.0.0`, another loopback spelling or a LAN address.
pub fn is_this_computer(host: &str) -> bool {
  let host = host
    .strip_prefix('[')
    .and_then(|inside| inside.strip_suffix(']'))
    .unwrap_or(host);
  THIS_COMPUTER_HOSTS.iter().any(|local| local.eq_ignore_ascii_case(host))
}

impl fmt::Display for DecisionServerUrl {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for DecisionServerUrl {
  type Error = DecisionServerUrlError;

  fn try_from(value: String) -> Result<Self, DecisionServerUrlError> {
    Self::new(&value)
  }
}

impl From<DecisionServerUrl> for String {
  fn from(url: DecisionServerUrl) -> Self {
    url.0
  }
}

/// A project's connection to a decision model's server: its name, base URL, the project secret holding its API key
/// if the server takes one, and the model ids its last check listed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionConnection {
  id: DecisionConnectionId,
  name: Name,
  base_url: DecisionServerUrl,
  key: Option<SecretId>,
  models: Vec<ModelId>,
}

impl DecisionConnection {
  /// A connection no check has listed models for yet.
  pub const fn new(id: DecisionConnectionId, name: Name, base_url: DecisionServerUrl, key: Option<SecretId>) -> Self {
    Self {
      id,
      name,
      base_url,
      key,
      models: Vec::new(),
    }
  }

  /// This connection with the models its last check listed.
  #[must_use]
  pub fn with_models(mut self, models: Vec<ModelId>) -> Self {
    self.record_models(models);
    self
  }

  pub const fn id(&self) -> DecisionConnectionId {
    self.id
  }

  pub const fn name(&self) -> &Name {
    &self.name
  }

  pub const fn base_url(&self) -> &DecisionServerUrl {
    &self.base_url
  }

  /// The project secret holding the API key; `None` for a server that takes none.
  pub const fn key(&self) -> Option<SecretId> {
    self.key
  }

  /// The model ids the last check listed, once each, in the server's order.
  pub fn models(&self) -> &[ModelId] {
    &self.models
  }

  pub fn is_on_this_computer(&self) -> bool {
    self.base_url.is_on_this_computer()
  }

  pub fn rename(&mut self, name: Name) {
    self.name = name;
  }

  /// Points the connection at another server or key. The listed models came from the previous server with the
  /// previous key, so another base URL or key drops them.
  pub fn set_server(&mut self, base_url: DecisionServerUrl, key: Option<SecretId>) {
    if base_url != self.base_url || key != self.key {
      self.models.clear();
    }
    self.base_url = base_url;
    self.key = key;
  }

  /// Replaces the listed models with those a check listed, kept once each in the order given.
  pub fn record_models(&mut self, models: Vec<ModelId>) {
    self.models.clear();
    for model in models {
      if !self.models.contains(&model) {
        self.models.push(model);
      }
    }
  }
}
