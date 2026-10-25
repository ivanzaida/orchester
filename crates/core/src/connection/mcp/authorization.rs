//! What an HTTP server is sent with every request so that it lets the agent in: nothing, a bearer token, Basic
//! credentials (a user name and a password, which the invocation encodes, or a credential the person pasted whole),
//! or a header of another name for servers that take another scheme (`X-API-Key`, `Authorization: Token …`). The
//! part that is a credential is always a project secret, named by its id; its value is read only when an agent
//! starts (`connection::invocation::HttpAuthorization`).

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::id::SecretId;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum McpAuthorizationError {
  #[error("Basic authentication needs a user name")]
  BlankUsername,
  /// RFC 7617: the user name ends at the first colon.
  #[error("a Basic user name must not contain a colon")]
  UsernameColon,
  #[error("a Basic user name must not contain a control character")]
  UsernameControl,
  #[error("a Basic user name is at most {max} characters long", max = BasicUsername::MAX_CHARS)]
  UsernameTooLong,
  #[error("a custom header needs a name")]
  BlankHeader,
  /// RFC 9110 `token`: letters, digits and ``!#$%&'*+-.^_`|~``.
  #[error("`{0}` is not a header name")]
  HeaderInvalid(String),
  #[error("a header name is at most {max} characters long", max = McpHeaderName::MAX_CHARS)]
  HeaderTooLong,
  /// A header the CLI sets itself to speak MCP over HTTP.
  #[error("the agent's CLI sets the header `{0}` itself")]
  HeaderReserved(String),
}

/// How an HTTP server is authorized.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scheme", rename_all = "snake_case")]
pub enum McpAuthorization {
  /// Nothing is sent.
  #[default]
  None,
  /// `Authorization: Bearer <token>`.
  Bearer { token: SecretId },
  /// `Authorization: Basic <base64 of username:password>`.
  Basic {
    username: BasicUsername,
    password: SecretId,
  },
  /// `Authorization: Basic <credential>`, the credential pasted whole: its base64 part, or the header's whole value
  /// starting with `Basic `.
  BasicCredential { credential: SecretId },
  /// `<name>: <value>`, the value whole.
  Header { name: McpHeaderName, value: SecretId },
}

impl McpAuthorization {
  /// The project secret sent; none for [`Self::None`].
  pub const fn secret(&self) -> Option<SecretId> {
    match self {
      Self::None => None,
      Self::Bearer { token: secret }
      | Self::Basic { password: secret, .. }
      | Self::BasicCredential { credential: secret }
      | Self::Header { value: secret, .. } => Some(*secret),
    }
  }
}

/// The user name of Basic credentials: no colon (RFC 7617) and no control character. It is not a secret: the
/// inspector shows it, and only the password is kept in the keychain.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BasicUsername(String);

impl BasicUsername {
  pub const MAX_CHARS: usize = 256;

  /// Surrounding whitespace is not part of the name.
  pub fn new(value: &str) -> Result<Self, McpAuthorizationError> {
    let name = value.trim();
    if name.is_empty() {
      return Err(McpAuthorizationError::BlankUsername);
    }
    if name.contains(':') {
      return Err(McpAuthorizationError::UsernameColon);
    }
    if name.chars().any(char::is_control) {
      return Err(McpAuthorizationError::UsernameControl);
    }
    if name.chars().count() > Self::MAX_CHARS {
      return Err(McpAuthorizationError::UsernameTooLong);
    }
    Ok(Self(name.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl TryFrom<String> for BasicUsername {
  type Error = McpAuthorizationError;

  fn try_from(value: String) -> Result<Self, McpAuthorizationError> {
    Self::new(&value)
  }
}

impl From<BasicUsername> for String {
  fn from(name: BasicUsername) -> Self {
    name.0
  }
}

/// The headers an MCP client over streamable HTTP sets itself, which a server's authorization must not replace.
const RESERVED: [&str; 8] = [
  "accept",
  "connection",
  "content-length",
  "content-type",
  "host",
  "mcp-protocol-version",
  "mcp-session-id",
  "transfer-encoding",
];

/// The name of the header an HTTP server is sent, e.g. `X-API-Key`: an RFC 9110 token, written as entered.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct McpHeaderName(String);

impl McpHeaderName {
  pub const MAX_CHARS: usize = 64;

  /// Surrounding whitespace is not part of the name.
  pub fn new(value: &str) -> Result<Self, McpAuthorizationError> {
    let name = value.trim();
    if name.is_empty() {
      return Err(McpAuthorizationError::BlankHeader);
    }
    if name.chars().count() > Self::MAX_CHARS {
      return Err(McpAuthorizationError::HeaderTooLong);
    }
    if !name
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(c))
    {
      return Err(McpAuthorizationError::HeaderInvalid(name.to_owned()));
    }
    if RESERVED.iter().any(|reserved| reserved.eq_ignore_ascii_case(name)) {
      return Err(McpAuthorizationError::HeaderReserved(name.to_owned()));
    }
    Ok(Self(name.to_owned()))
  }

  /// `Authorization`, the header of the bearer and Basic schemes.
  pub fn authorization() -> Self {
    Self("Authorization".to_owned())
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl TryFrom<String> for McpHeaderName {
  type Error = McpAuthorizationError;

  fn try_from(value: String) -> Result<Self, McpAuthorizationError> {
    Self::new(&value)
  }
}

impl From<McpHeaderName> for String {
  fn from(name: McpHeaderName) -> Self {
    name.0
  }
}
