//! What a Share tells the person beyond "shared" (`docs/design/team-configuration.md`, edge case 12): what this
//! computer could not fill in for the team's configuration ([`NotSet`]) and what Share did that they should know
//! ([`ShareNote`]). The project is shared by then, so none of it undoes the Share; each says what to set or what
//! changed, in words that never repeat a secret's value.

use orchester_core::secret::SecretName;

use crate::messages::Message;

/// Something this computer could not set after the Share, so the team's configuration is not ready here until the
/// person sets it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotSet {
  /// The program of a local MCP server started by a name on the search path was found nowhere on it: its path is to
  /// be set.
  McpProgramPath { server: String },
  /// The path of the program of an MCP server could not be saved.
  McpProgramKept { server: String },
  /// A team MCP server that was on could not be switched on here.
  McpOn { server: String },
  /// The connection a profile ran on could not be kept as its pick.
  Connection { profile: String },
  /// The secret has no value on this computer (none in its keychain, or a blank one), so none was carried.
  SecretValue { secret: SecretName },
  /// The value of the secret could not be saved.
  SecretKept { secret: SecretName },
}

impl NotSet {
  pub fn message(&self) -> Message {
    match self {
      Self::McpProgramPath { server } => {
        Message::new("sharing.shared.notSet.mcpProgramPath").with("server", server.clone())
      }
      Self::McpProgramKept { server } => {
        Message::new("sharing.shared.notSet.mcpProgramKept").with("server", server.clone())
      }
      Self::McpOn { server } => Message::new("sharing.shared.notSet.mcpOn").with("server", server.clone()),
      Self::Connection { profile } => Message::new("sharing.shared.notSet.connection").with("profile", profile.clone()),
      Self::SecretValue { secret } => {
        Message::new("sharing.shared.notSet.secretValue").with("secret", secret.as_str().to_owned())
      }
      Self::SecretKept { secret } => {
        Message::new("sharing.shared.notSet.secretKept").with("secret", secret.as_str().to_owned())
      }
    }
  }
}

/// Something Share did that differs from what the person had locally.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShareNote {
  /// The plain-text values of a stdio server's variables are team secrets now: the team names them and each member
  /// sets their value. This computer kept its own values.
  VariablesAreSecrets { server: String, variables: Vec<String> },
  /// An older version of a profile chose an MCP server this computer no longer has; the shared copy leaves it out.
  GoneServerLeftOut { profile: String, version: u32 },
  /// An older version of a profile chose an MCP server that cannot be a team one; the shared copy leaves it out.
  UnsharedServerLeftOut {
    profile: String,
    version: u32,
    server: String,
  },
}

impl ShareNote {
  pub fn message(&self) -> Message {
    match self {
      Self::VariablesAreSecrets { server, variables } => Message::new("sharing.shared.notes.variablesAreSecrets")
        .with("server", server.clone())
        .with("variables", variables.join(", ")),
      Self::GoneServerLeftOut { profile, version } => Message::new("sharing.shared.notes.goneServerLeftOut")
        .with("profile", profile.clone())
        .with("version", version.to_string()),
      Self::UnsharedServerLeftOut {
        profile,
        version,
        server,
      } => Message::new("sharing.shared.notes.unsharedServerLeftOut")
        .with("profile", profile.clone())
        .with("version", version.to_string())
        .with("server", server.clone()),
    }
  }
}
