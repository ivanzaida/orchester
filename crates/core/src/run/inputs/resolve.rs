//! What a run executes as this computer resolves its inputs (team authority §7): each pinned body, a pinned profile on
//! the connection and with the MCP servers its bindings name, what a person here narrowed, and the task as it was at
//! the start. Nothing here reads the current task or resource definitions.

use std::path::Path;

use super::{InputsError, InputsOrigin, LocalBindings, ProfileBinding, RunInputs};
use crate::{
  connection::{Connection, mcp::McpServerSelection},
  id::{ConnectionId, ResourceId, TaskId},
  permission::AgentPermissions,
  project::{ProjectKey, TaskKey},
  resource::{AgentProfile, Instruction, StepKey, TeamMcpServer, TeamTool},
  run::{PinnedVersion, bundle::PinnedBody},
  sharing::{ConnectionRole, TaskRevision},
  task::{Task, TaskDetails, TaskStatus, organization::TaskOrganization},
  time::Timestamp,
};

/// The id a team MCP server runs under in a profile's selection: its resource id as a connection id, which no local
/// MCP server of this computer carries.
pub fn team_mcp_id(server: ResourceId) -> ConnectionId {
  ConnectionId::from_uuid(server.as_uuid())
}

/// The MCP servers the bindings bind, in their order, over all profiles: the local servers an older run bound
/// ([`LocalBindings::local_mcp`]), then the team servers of the profiles (`CanonicalExecutionInputs::mcp_servers`),
/// which this computer binds to a program and values of its own ([`LocalBindings::team_mcp`]).
pub(super) fn mcp_servers(bindings: &LocalBindings) -> Result<McpServerSelection, InputsError> {
  let servers = bindings
    .local_mcp
    .iter()
    .copied()
    .chain(bindings.team_mcp.iter().map(|bound| team_mcp_id(bound.server)))
    .collect();
  McpServerSelection::new(servers).map_err(InputsError::McpServers)
}

/// The MCP servers `binding`'s profile names: a selection of bound servers, checked when the inputs are made.
pub(super) fn profile_mcp_servers(
  bindings: &LocalBindings,
  binding: &ProfileBinding,
) -> Result<McpServerSelection, InputsError> {
  let bound = mcp_servers(bindings)?;
  if let Some(unbound) = binding.mcp_servers.iter().find(|server| !bound.ids().contains(server)) {
    return Err(InputsError::UnboundMcpServer {
      profile: binding.resource,
      server: *unbound,
    });
  }
  McpServerSelection::new(binding.mcp_servers.clone()).map_err(InputsError::McpServers)
}

fn body<B>(list: &[PinnedBody<B>], pinned: PinnedVersion) -> Option<&B> {
  list.iter().find(|body| body.pinned() == pinned).map(PinnedBody::body)
}

impl RunInputs {
  /// Whether the team server accepted the inputs.
  pub const fn is_team(&self) -> bool {
    matches!(self.bundle.origin(), InputsOrigin::Team(_))
  }

  /// The pinned profile version `pinned` as this computer runs it: the accepted body on the connection its binding
  /// names, giving its agents only the MCP servers its own binding names, without extra writable folders. `None` when
  /// the inputs do not pin it.
  pub fn profile(&self, pinned: PinnedVersion) -> Option<AgentProfile> {
    let accepted = body(self.bundle.inputs().profiles(), pinned)?;
    let binding = self
      .bindings
      .profiles
      .iter()
      .find(|binding| binding.resource == pinned.resource)?;
    let servers = profile_mcp_servers(&self.bindings, binding)
      .expect("the profiles' MCP servers were checked when the inputs were made");
    Some(accepted.bound(binding.connection, servers))
  }

  /// Refuses a shared profile's connection as unbound when its kind differs from the recorded reference.
  /// An absent requirement preserves runs recorded before this check.
  pub fn check_connection(&self, profile: ResourceId, connection: &Connection) -> Result<(), InputsError> {
    let binding = self
      .bindings
      .profiles
      .iter()
      .find(|binding| binding.resource == profile)
      .ok_or(InputsError::MissingBinding(profile))?;
    if self.is_team()
      && binding
        .required_cli
        .is_some_and(|cli| ConnectionRole::of(connection.kind()) != ConnectionRole::Agent(cli))
    {
      return Err(InputsError::MissingBinding(profile));
    }
    Ok(())
  }

  pub fn instruction(&self, pinned: PinnedVersion) -> Option<&Instruction> {
    body(self.bundle.inputs().instructions(), pinned)
  }

  pub fn tool(&self, pinned: PinnedVersion) -> Option<&TeamTool> {
    body(self.bundle.inputs().tools(), pinned)
  }

  /// What a person here narrowed the agents of step `step` to; `None` when they did not.
  pub fn narrowed(&self, step: &StepKey) -> Option<&AgentPermissions> {
    self
      .consent
      .narrowed
      .iter()
      .find(|narrowed| narrowed.step == *step)
      .map(|narrowed| &narrowed.permissions)
  }

  /// The program this computer runs the team MCP server selected as `id` with, and the pinned version of the server;
  /// `None` when `id` is no team MCP server of this run. The program is `None` for a remote server.
  pub fn team_mcp_server(&self, id: ConnectionId) -> Option<(&TeamMcpServer, Option<&Path>)> {
    let bound = self
      .bindings
      .team_mcp
      .iter()
      .find(|bound| team_mcp_id(bound.server) == id)?;
    let pinned = self
      .bundle
      .inputs()
      .mcp_servers()
      .iter()
      .find(|pinned| pinned.pinned().resource == bound.server)?;
    Some((pinned.body(), bound.program.as_deref()))
  }

  /// The ids the run's team MCP servers are selected under, which no local MCP server of this computer answers to.
  pub fn team_mcp_ids(&self) -> Vec<ConnectionId> {
    self
      .bindings
      .team_mcp
      .iter()
      .map(|bound| team_mcp_id(bound.server))
      .collect()
  }

  /// The task `id` of the project keyed `project` as the run started it: its title, brief and criteria, the pinned
  /// pipeline at its version, the limits and what the run delivers. A team task is in its accepted module, at the
  /// revision the work started from.
  pub fn task(&self, id: TaskId, project: &ProjectKey, created_at: Timestamp) -> Task {
    let content = self.bundle.task();
    let pipeline = self.bundle.inputs().pipeline().pinned();
    let details = TaskDetails {
      title: self.bundle.task_title(),
      brief: content.brief.clone(),
      acceptance_criteria: content.criteria.clone(),
      pipeline: Some(pipeline.resource),
      limits: self.bundle.limits(),
    };
    let key = TaskKey::new(project.clone(), self.bundle.task_number());
    let task = Task::from_stored(id, key, details, TaskStatus::Queued, Some(pipeline.number), created_at)
      .with_organization(TaskOrganization::default().with_delivers(Some(self.bundle.delivery())));
    match self.bundle.origin() {
      InputsOrigin::Local => task.with_revision(TaskRevision::FIRST),
      InputsOrigin::Team(accepted) => task
        .with_module(accepted.module)
        .with_revision(accepted.definition_revision),
    }
  }
}
