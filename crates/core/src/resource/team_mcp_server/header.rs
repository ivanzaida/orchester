//! A header a remote team MCP server is sent a secret in: the header's name, the team secret its value comes from and
//! a fixed prefix. The body holds names and plain text only; the value is the prefix followed by the secret's value,
//! put together on each computer when an agent starts.

use std::fmt;

use serde::{
  Deserialize, Serialize,
  de::{self, MapAccess, Visitor, value::MapAccessDeserializer},
};
use thiserror::Error;

use super::TeamMcpServerError;
use crate::{connection::mcp::McpHeaderName, secret::SecretName};

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum HeaderPrefixError {
  /// A prefix of spaces only would start the value with nothing a server can read; leave the prefix out instead.
  #[error("a header prefix must have a visible character; leave it out for none")]
  Blank,
  #[error("a header prefix is at most {max} characters long", max = HeaderPrefix::MAX_CHARS)]
  TooLong,
  #[error("a header prefix must not contain a control character")]
  Control,
}

/// The fixed start of a secret header's value, e.g. `Bearer `. Plain text the team sees and keeps, never a value: it
/// is short, and the errors do not repeat it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct HeaderPrefix(String);

impl HeaderPrefix {
  pub const MAX_CHARS: usize = 32;

  /// Kept as written: the space that ends `Bearer ` is part of it.
  pub fn new(value: &str) -> Result<Self, HeaderPrefixError> {
    if value.trim().is_empty() {
      return Err(HeaderPrefixError::Blank);
    }
    if value.chars().count() > Self::MAX_CHARS {
      return Err(HeaderPrefixError::TooLong);
    }
    if value.chars().any(char::is_control) {
      return Err(HeaderPrefixError::Control);
    }
    Ok(Self(value.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl TryFrom<String> for HeaderPrefix {
  type Error = HeaderPrefixError;

  fn try_from(value: String) -> Result<Self, HeaderPrefixError> {
    Self::new(&value)
  }
}

impl From<HeaderPrefix> for String {
  fn from(prefix: HeaderPrefix) -> Self {
    prefix.0
  }
}

/// `header: <prefix><value of the team secret>`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TeamMcpSecretHeader {
  header: McpHeaderName,
  secret: SecretName,
  #[serde(skip_serializing_if = "Option::is_none")]
  prefix: Option<HeaderPrefix>,
}

impl TeamMcpSecretHeader {
  pub const fn new(header: McpHeaderName, secret: SecretName, prefix: Option<HeaderPrefix>) -> Self {
    Self { header, secret, prefix }
  }

  pub const fn header(&self) -> &McpHeaderName {
    &self.header
  }

  /// The team secret whose value this header is sent.
  pub const fn secret(&self) -> &SecretName {
    &self.secret
  }

  /// The text the value starts with; the header's value is this followed by the secret's value.
  pub const fn prefix(&self) -> Option<&HeaderPrefix> {
    self.prefix.as_ref()
  }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HeaderFields {
  header: McpHeaderName,
  secret: SecretName,
  #[serde(default)]
  prefix: Option<HeaderPrefix>,
}

/// Reads `{header, secret, prefix?}`. A bare header name, as bodies held before a header named its secret, is refused
/// with [`TeamMcpServerError::HeaderWithoutSecret`]: which secret it takes is not guessed.
impl<'de> Deserialize<'de> for TeamMcpSecretHeader {
  fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    deserializer.deserialize_any(HeaderVisitor)
  }
}

struct HeaderVisitor;

impl<'de> Visitor<'de> for HeaderVisitor {
  type Value = TeamMcpSecretHeader;

  fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str("a secret header: {header, secret, prefix?}")
  }

  fn visit_str<E: de::Error>(self, name: &str) -> Result<Self::Value, E> {
    Err(E::custom(TeamMcpServerError::HeaderWithoutSecret(name.to_owned())))
  }

  fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
    let fields = HeaderFields::deserialize(MapAccessDeserializer::new(map))?;
    Ok(TeamMcpSecretHeader::new(fields.header, fields.secret, fields.prefix))
  }
}
