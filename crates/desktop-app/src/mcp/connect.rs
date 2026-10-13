//! The commands Settings › Automation › *Connect an agent* gives to register the installed app's server once, as
//! checked with Claude Code 2.1.296 and Codex CLI 0.159.2 (`docs/design/app-mcp-connection.md`, "Client commands").
//! The window shows them with the token masked; Copy copies them with the token. The server is registered as
//! `orchester`, never `Orchester`, the name runs give Orchester's relay, whose table a user table of that name would
//! merge into. Claude Code has no project scope here: it would write the token into the repository's `.mcp.json`.

use std::num::NonZeroU16;

use super::server::endpoint;

/// The name agents know the server by.
pub const SERVER_NAME: &str = "orchester";
/// The user environment variable Codex reads the token from (`--bearer-token-env-var`), named apart from the
/// runner's `ORCHESTER_MCP_TOKEN_*` and never passed to agents in runs.
pub const TOKEN_VARIABLE: &str = "ORCHESTER_APP_MCP_TOKEN";

/// Where Claude Code makes the server available.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClaudeScope {
  /// In every folder (`--scope user`).
  #[default]
  User,
  /// In the folder the command runs in (`--scope local`).
  Local,
}

impl ClaudeScope {
  const fn flag(self) -> &'static str {
    match self {
      Self::User => "user",
      Self::Local => "local",
    }
  }
}

impl lurq::app::component::DevtoolsInspectable for ClaudeScope {}

/// `claude mcp add` for the server on `port`, sending `token`.
pub fn claude(port: NonZeroU16, scope: ClaudeScope, token: &str) -> String {
  format!(
    "claude mcp add --transport http --scope {} {SERVER_NAME} {} --header \"Authorization: Bearer {token}\"",
    scope.flag(),
    endpoint(port)
  )
}

/// `codex mcp add` for the server on `port`; Codex reads the token from [`TOKEN_VARIABLE`].
pub fn codex(port: NonZeroU16) -> String {
  format!(
    "codex mcp add {SERVER_NAME} --url {} --bearer-token-env-var {TOKEN_VARIABLE}",
    endpoint(port)
  )
}

/// Sets [`TOKEN_VARIABLE`] to `token` for the user, the way this platform keeps it for programs started later.
pub fn codex_variable(token: &str) -> String {
  platform::set_variable(token)
}

/// The `~/.codex/config.toml` entry that sends `token` without a variable.
pub fn codex_config(port: NonZeroU16, token: &str) -> String {
  format!(
    "[mcp_servers.{SERVER_NAME}]\nurl = \"{}\"\nhttp_headers = {{ Authorization = \"Bearer {token}\" }}",
    endpoint(port)
  )
}

/// How the user's environment variable is set: `setx` on Windows keeps it in the user's profile; on macOS
/// `launchctl setenv` reaches apps started from the Dock or Finder until the person signs out (Settings says to add
/// the same export to `~/.zshrc` for terminals).
mod platform {
  use super::TOKEN_VARIABLE;

  #[cfg(target_os = "macos")]
  pub fn set_variable(token: &str) -> String {
    format!("launchctl setenv {TOKEN_VARIABLE} {token}")
  }

  #[cfg(not(target_os = "macos"))]
  pub fn set_variable(token: &str) -> String {
    format!("setx {TOKEN_VARIABLE} {token}")
  }
}
