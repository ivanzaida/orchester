use std::{
  collections::{HashMap, HashSet},
  path::PathBuf,
};

use crate::{
  connection::AgentProvider,
  id::{ConnectionId, DecisionConnectionId, ResourceId, SecretId},
  secret::SecretName,
};

/// Where a team MCP server's program is on this computer, and whether the file is there. The caller looks at the
/// filesystem; the rule does no I/O.
#[derive(Clone, Debug)]
pub(super) struct ProgramFile {
  pub(super) path: PathBuf,
  pub(super) exists: bool,
}

/// What this computer has, as plain data: the answers to "is it ready here" come from it alone. A local project fills
/// it with the connection a profile names as that profile's pick, the program files of the MCP servers its profiles
/// choose and the values of the secrets its tools use.
#[derive(Clone, Debug, Default)]
pub struct Here {
  pub(super) connections: Vec<(ConnectionId, AgentProvider)>,
  pub(super) picks: HashMap<ResourceId, ConnectionId>,
  pub(super) decision_connections: Vec<DecisionConnectionId>,
  pub(super) decision_picks: HashMap<ResourceId, DecisionConnectionId>,
  pub(super) mcp_on: HashSet<ResourceId>,
  pub(super) programs: HashMap<ResourceId, ProgramFile>,
  pub(super) secrets: HashSet<SecretName>,
  pub(super) secrets_unreadable: HashSet<SecretName>,
  pub(super) local_programs: HashSet<ConnectionId>,
  pub(super) local_programs_moved: HashSet<ConnectionId>,
  pub(super) local_secrets: HashSet<SecretId>,
}

impl Here {
  /// A computer with nothing: no connection, no program, no secret value, no MCP server on.
  pub fn nothing() -> Self {
    Self::default()
  }

  /// A connection of this computer that runs `cli`.
  #[must_use]
  pub fn with_connection(mut self, connection: ConnectionId, cli: AgentProvider) -> Self {
    self.connections.push((connection, cli));
    self
  }

  /// The connection the person picked for team profile `profile`. A pick of a connection that is not here any more
  /// counts as no pick.
  #[must_use]
  pub fn with_pick(mut self, profile: ResourceId, connection: ConnectionId) -> Self {
    self.picks.insert(profile, connection);
    self
  }

  /// A decision connection of this computer.
  #[must_use]
  pub fn with_decision_connection(mut self, connection: DecisionConnectionId) -> Self {
    self.decision_connections.push(connection);
    self
  }

  /// The decision connection the person picked for team reviewer `reviewer`. A pick of a connection that is not here
  /// any more counts as no pick.
  #[must_use]
  pub fn with_decision_pick(mut self, reviewer: ResourceId, connection: DecisionConnectionId) -> Self {
    self.decision_picks.insert(reviewer, connection);
    self
  }

  /// Team MCP server `server` is switched on here.
  #[must_use]
  pub fn with_mcp_on(mut self, server: ResourceId) -> Self {
    self.mcp_on.insert(server);
    self
  }

  /// The program path set for team MCP server `server`, and whether the file `exists` on disk now.
  #[must_use]
  pub fn with_program(mut self, server: ResourceId, path: PathBuf, exists: bool) -> Self {
    self.programs.insert(server, ProgramFile { path, exists });
    self
  }

  /// A value is set here for the team secret `name`. Only the name is kept.
  #[must_use]
  pub fn with_secret(mut self, name: SecretName) -> Self {
    self.secrets.insert(name);
    self
  }

  /// The keychain did not say whether a value is set here for the team secret `name`. Only the name is kept; a name
  /// that also has a value counts as set.
  #[must_use]
  pub fn with_secret_unreadable(mut self, name: SecretName) -> Self {
    self.secrets_unreadable.insert(name);
    self
  }

  /// The program of the local project's MCP server `server` is a file a run starts.
  #[must_use]
  pub fn with_local_program(mut self, server: ConnectionId) -> Self {
    self.local_programs.insert(server);
    self
  }

  /// The program of the local project's MCP server `server` is a file, but a run would refuse to start it
  /// ([`LocalProgramProblem::Moved`](super::LocalProgramProblem::Moved)).
  #[must_use]
  pub fn with_local_program_moved(mut self, server: ConnectionId) -> Self {
    self.local_programs_moved.insert(server);
    self
  }

  /// The project secret `secret` has a value here. Only the id is kept.
  #[must_use]
  pub fn with_local_secret(mut self, secret: SecretId) -> Self {
    self.local_secrets.insert(secret);
    self
  }
}
