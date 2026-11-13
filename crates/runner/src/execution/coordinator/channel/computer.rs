//! The computer's resources over the channel: its connections, what a profile runs on there, its home folder and
//! environment cross as JSON to a thread that answers with the computer the scheduler had (the local one), and back.
//! Its secrets, their values and its MCP servers and its decision model connections are answered by that computer on
//! the executor's side (see the module above).

use std::{path::PathBuf, sync::Arc};

use orchester_core::{
  computer::{ComputerConnection, ComputerError},
  connection::{DecisionConnection, mcp::McpServer},
  id::{ConnectionId, DecisionConnectionId, ProjectId, ResourceId, SecretId},
  memory::{ScreenUnavailable, SecretScreen},
  resource::CommandEnvironment,
  secret::{Secret, SecretName, SecretValue, VaultError},
};

use super::{Calls, Reply, Wire, crossed_whole, received, whole};
use crate::computer::ComputerResources;

type Backend = Arc<dyn ComputerResources>;

pub(super) struct ChannelComputer {
  wire: Wire<Backend>,
  /// The same computer, for what never crosses.
  local: Backend,
}

impl ChannelComputer {
  /// The resources `computer` answers, over the channel where they can cross; each crossing is counted in `calls`.
  pub(super) fn over(computer: Backend, calls: Arc<Calls>) -> Self {
    Self {
      wire: Wire::open(computer.clone(), serve, calls),
      local: computer,
    }
  }
}

/// Answers the request `text` holds for `method` with `computer`'s method of that name.
fn serve(computer: &mut Backend, method: &str, text: &str) -> Reply {
  match method {
    "connection" => whole(&computer.connection(received(text))),
    "connections" => {
      received::<()>(text);
      whole(&computer.connections())
    }
    "profile_connection" => {
      let (profile, named) = received(text);
      whole(&computer.profile_connection(profile, named))
    }
    "home_folder" => {
      received::<()>(text);
      whole(&computer.home_folder())
    }
    "environment" => whole(&computer.environment(received(text))),
    other => panic!("the computer's resources have no request `{other}` over the channel"),
  }
}

impl ComputerResources for ChannelComputer {
  fn connection(&self, id: ConnectionId) -> Result<Option<ComputerConnection>, ComputerError> {
    crossed_whole(self.wire.ask("connection", &id))
  }

  fn connections(&self) -> Result<Vec<ComputerConnection>, ComputerError> {
    crossed_whole(self.wire.ask("connections", &()))
  }

  fn profile_connection(&self, profile: ResourceId, named: ConnectionId) -> Result<ConnectionId, ComputerError> {
    crossed_whole(self.wire.ask("profile_connection", &(profile, named)))
  }

  fn home_folder(&self) -> Option<PathBuf> {
    crossed_whole(self.wire.ask("home_folder", &()))
  }

  fn decision_connection(&self, id: DecisionConnectionId) -> Result<Option<DecisionConnection>, ComputerError> {
    self.local.decision_connection(id)
  }

  fn mcp_server(&self, id: ConnectionId) -> Result<Option<McpServer>, ComputerError> {
    self.local.mcp_server(id)
  }

  fn mcp_servers(&self) -> Result<Vec<McpServer>, ComputerError> {
    self.local.mcp_servers()
  }

  fn secret(&self, id: SecretId) -> Result<Option<Secret>, ComputerError> {
    self.local.secret(id)
  }

  fn secrets(&self) -> Result<Vec<Secret>, ComputerError> {
    self.local.secrets()
  }

  fn secret_value(&self, project: ProjectId, secret: &Secret) -> Result<SecretValue, VaultError> {
    self.local.secret_value(project, secret)
  }

  fn team_secret_value(&self, name: &SecretName) -> Result<Option<SecretValue>, ComputerError> {
    self.local.team_secret_value(name)
  }

  fn secret_screen(&self, project: ProjectId, secrets: &[Secret]) -> Result<SecretScreen, ScreenUnavailable> {
    self.local.secret_screen(project, secrets)
  }

  fn environment(&self, project: CommandEnvironment) -> Result<CommandEnvironment, ComputerError> {
    crossed_whole(self.wire.ask("environment", &project))
  }
}
