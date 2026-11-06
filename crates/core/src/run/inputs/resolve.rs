//! What a run executes as this computer resolves its inputs (team authority §7): each pinned body, a pinned profile on
//! the connection and with the MCP servers its bindings name, what a person here narrowed, and the task as it was at
//! the start. Nothing here reads the current task or resource definitions.

use super::{BoundReference, InputsError, InputsOrigin, LocalBindings, ProfileBinding, RunInputs};
use crate::{
  connection::{Connection, mcp::McpServerSelection},
  id::{ConnectionId, ResourceId, SecretId, TaskId},
  permission::AgentPermissions,
  project::{ProjectKey, TaskKey},
  resource::{AgentProfile, Instruction, StepKey, Tool},
  run::{PinnedVersion, bundle::PinnedBody},
  sharing::{ConnectionRole, TaskRevision},
  task::{Task, TaskDetails, TaskStatus, organization::TaskOrganization},
  time::Timestamp,
};

/// The MCP servers the bindings bind, in their order, over all profiles. A bundle names no MCP server of a profile:
/// they are this computer's, and its bindings choose them.
pub(super) fn mcp_servers(bindings: &LocalBindings) -> Result<McpServerSelection, InputsError> {
  let servers = bindings
    .references
    .iter()
    .filter_map(|reference| match reference {
      BoundReference::McpServer { local, .. } => Some(*local),
      BoundReference::Connection { .. } | BoundReference::Secret { .. } => None,
    })
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

  pub fn tool(&self, pinned: PinnedVersion) -> Option<&Tool> {
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

  /// The local connections the bindings run on: each profile's, and each connection reference's.
  pub fn bound_connections(&self) -> Vec<ConnectionId> {
    let references = self.bindings.references.iter().filter_map(|reference| match reference {
      BoundReference::Connection { local, .. } => Some(*local),
      BoundReference::McpServer { .. } | BoundReference::Secret { .. } => None,
    });
    self
      .bindings
      .profiles
      .iter()
      .map(|binding| binding.connection)
      .chain(references)
      .collect()
  }

  /// The local MCP servers the bindings bind for the run, over all its profiles.
  pub fn bound_mcp_servers(&self) -> McpServerSelection {
    mcp_servers(&self.bindings).expect("the bound MCP servers were checked when the inputs were made")
  }

  /// The local secrets the bindings name; their values are looked up when an agent or tool starts, never kept here.
  pub fn bound_secrets(&self) -> Vec<SecretId> {
    self
      .bindings
      .references
      .iter()
      .filter_map(|reference| match reference {
        BoundReference::Secret { local, .. } => Some(*local),
        BoundReference::Connection { .. } | BoundReference::McpServer { .. } => None,
      })
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
        .with_module(Some(accepted.module))
        .with_revision(accepted.definition_revision),
    }
  }
}
