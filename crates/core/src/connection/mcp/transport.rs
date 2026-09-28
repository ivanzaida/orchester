//! How an agent CLI reaches an MCP server: a program it starts and talks to over standard input and
//! output (stdio), or a URL it talks to over streamable HTTP. Both CLIs support both
//! (`codex mcp add --url`, Claude Code's `"type": "http"`; `docs/design/orchester-workspace.md`, "MCP servers").

use std::{
  fmt,
  path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
  id::SecretId,
  resource::{CommandArg, CommandProgram},
};

/// Longest URL or working directory, in characters.
const MAX_CHARS: usize = 2048;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TransportError {
  #[error("an MCP server needs a URL")]
  BlankUrl,
  #[error("the URL of an MCP server starts with http:// or https:// and names a host")]
  UrlScheme,
  /// A user name or password in the URL would be passed to the agent CLI in its arguments, where
  /// any process can read it. A credential belongs in a project secret.
  #[error("the URL of an MCP server must not contain a user name or password")]
  UrlCredentials,
  #[error("`{0}` contains a space or a control character")]
  Whitespace(String),
  #[error("`{0}` is longer than {MAX_CHARS} characters")]
  TooLong(String),
  #[error("the working directory must not be blank")]
  BlankWorkingDirectory,
  #[error("the working directory `{0}` leaves the worktree")]
  OutsideWorktree(String),
}

/// How the server is reached.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpTransport {
  /// A program the CLI starts with the agent and ends with it.
  Stdio(StdioServer),
  /// A server already running somewhere, reached over streamable HTTP.
  Http(HttpServer),
}

/// A program the CLI starts: a name looked up on the `PATH`, a path in the worktree or an absolute
/// path, its arguments as they are, and optionally the folder it starts in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StdioServer {
  program: CommandProgram,
  args: Vec<CommandArg>,
  working_directory: Option<WorkingDirectory>,
}

impl StdioServer {
  pub const fn new(
    program: CommandProgram,
    args: Vec<CommandArg>,
    working_directory: Option<WorkingDirectory>,
  ) -> Self {
    Self {
      program,
      args,
      working_directory,
    }
  }

  pub const fn program(&self) -> &CommandProgram {
    &self.program
  }

  pub fn args(&self) -> &[CommandArg] {
    &self.args
  }

  /// Where the program starts; `None`: in the worktree.
  pub const fn working_directory(&self) -> Option<&WorkingDirectory> {
    self.working_directory.as_ref()
  }
}

/// A server reached over streamable HTTP, with a project secret as its bearer token if it needs one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpServer {
  url: McpUrl,
  bearer: Option<SecretId>,
}

impl HttpServer {
  pub const fn new(url: McpUrl, bearer: Option<SecretId>) -> Self {
    Self { url, bearer }
  }

  pub const fn url(&self) -> &McpUrl {
    &self.url
  }

  /// The secret sent as `Authorization: Bearer <value>`.
  pub const fn bearer(&self) -> Option<SecretId> {
    self.bearer
  }
}

/// The URL of an HTTP server: `http://` or `https://`, a host, no user name or password, no
/// whitespace.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct McpUrl(String);

impl McpUrl {
  /// Surrounding whitespace is not part of the URL.
  pub fn new(value: &str) -> Result<Self, TransportError> {
    let url = value.trim();
    if url.is_empty() {
      return Err(TransportError::BlankUrl);
    }
    check_text(url)?;
    let lower = url.to_ascii_lowercase();
    let rest = ["http://", "https://"]
      .into_iter()
      .find_map(|scheme| lower.strip_prefix(scheme).map(|_| &url[scheme.len()..]))
      .ok_or(TransportError::UrlScheme)?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains('@') {
      return Err(TransportError::UrlCredentials);
    }
    if authority.is_empty() || authority.starts_with(':') {
      return Err(TransportError::UrlScheme);
    }
    Ok(Self(url.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }

  /// The host and port, as the connection list shows where a server is (`mcp.example.dev`).
  pub fn host(&self) -> &str {
    let rest = self.0.split_once("://").map_or(self.0.as_str(), |(_, rest)| rest);
    rest.split(['/', '?', '#']).next().unwrap_or(rest)
  }
}

impl fmt::Display for McpUrl {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for McpUrl {
  type Error = TransportError;

  fn try_from(value: String) -> Result<Self, TransportError> {
    Self::new(&value)
  }
}

impl From<McpUrl> for String {
  fn from(url: McpUrl) -> Self {
    url.0
  }
}

/// The folder a stdio server starts in: a path relative to the worktree, which it must not leave,
/// or an absolute path.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct WorkingDirectory(String);

impl WorkingDirectory {
  /// Surrounding whitespace is not part of the path.
  pub fn new(value: &str) -> Result<Self, TransportError> {
    let path = value.trim();
    if path.is_empty() {
      return Err(TransportError::BlankWorkingDirectory);
    }
    if path.chars().any(char::is_control) {
      return Err(TransportError::Whitespace(path.to_owned()));
    }
    if path.chars().count() > MAX_CHARS {
      return Err(TransportError::TooLong(path.to_owned()));
    }
    let relative = !(Path::new(path).is_absolute() || Path::new(path).has_root());
    // Both separators count on every platform, so a server means the same on Windows and macOS.
    if relative && path.split(['/', '\\']).any(|segment| segment == "..") {
      return Err(TransportError::OutsideWorktree(path.to_owned()));
    }
    Ok(Self(path.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }

  /// The folder, for an invocation working in `worktree`.
  pub fn resolve(&self, worktree: &Path) -> PathBuf {
    worktree.join(&self.0)
  }
}

impl TryFrom<String> for WorkingDirectory {
  type Error = TransportError;

  fn try_from(value: String) -> Result<Self, TransportError> {
    Self::new(&value)
  }
}

impl From<WorkingDirectory> for String {
  fn from(directory: WorkingDirectory) -> Self {
    directory.0
  }
}

fn check_text(value: &str) -> Result<(), TransportError> {
  if value.chars().any(|c| c.is_whitespace() || c.is_control()) {
    return Err(TransportError::Whitespace(value.to_owned()));
  }
  if value.chars().count() > MAX_CHARS {
    return Err(TransportError::TooLong(value.to_owned()));
  }
  Ok(())
}
