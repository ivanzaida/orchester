//! Who executes each step of a run, as the task page shows it ([`super::detail::run_view`]): an agent step's role,
//! the CLI and model API its connection reaches, its model and its permissions in the run; a tool step's tool.

use std::sync::Arc;

use orchester_core::{
  connection::{AgentConnection, AgentProvider, EngineVendor},
  id::ConnectionId,
  resource::{AgentProfile, StepKey},
  run::{Invocation, PinnedVersion, Run, StepExecutor},
  store::{ConnectionStore, ResourceStore},
};
use orchester_store::{ProjectDb, StoreError};

use super::permissions::{RunPermissions, run_permissions};

/// Who executes a step (its agent role, or its tool's name), what it reaches, its model and its
/// permissions in the run.
pub(super) type Executed = (Option<Arc<str>>, Reached, Option<Arc<str>>, Option<RunPermissions>);

/// [`Executed`] of step `key`.
pub(super) fn executed(
  db: &ProjectDb,
  run: (&Run, &[Invocation]),
  key: &StepKey,
  executor: Option<&StepExecutor>,
) -> Result<Executed, StoreError> {
  Ok(match executor {
    Some(StepExecutor::Agent { profile, model, .. }) => {
      let (role, reached, permissions) = agent_step(db, run, key, *profile)?;
      (role, reached, Some(Arc::<str>::from(model.as_str())), permissions)
    }
    Some(StepExecutor::Tool { tool }) => {
      let name = db
        .resource(tool.resource)?
        .map(|tool| Arc::<str>::from(tool.name().as_str()));
      (name, Reached::default(), None, None)
    }
    None => (None, Reached::default(), None, None),
  })
}

/// The agent CLI of a connection, for a native one the base URL of its model API, and who receives the
/// prompts.
#[derive(Default)]
pub(super) struct Reached {
  pub(super) provider: Option<AgentProvider>,
  pub(super) endpoint: Option<Arc<str>>,
  pub(super) vendor: Option<EngineVendor>,
}

/// An agent step's role, CLI and model API, and permissions.
type AgentStep = (Option<Arc<str>>, Reached, Option<RunPermissions>);

/// The agent role, the CLI and the permissions of agent step `step`, which runs on `profile`; none
/// of them once the profile version is gone.
fn agent_step(
  db: &ProjectDb,
  run: (&Run, &[Invocation]),
  step: &StepKey,
  profile: PinnedVersion,
) -> Result<AgentStep, StoreError> {
  let Some(version) = db.version::<AgentProfile>(profile.resource, profile.number)? else {
    return Ok((None, Reached::default(), None));
  };
  let role = Arc::<str>::from(version.body().role().as_str());
  let reached = reached(db, version.body().connection())?;
  let permissions = run_permissions(db, run, step, (profile.resource, &version), reached.provider)?;
  Ok((Some(role), reached, permissions))
}

pub(super) fn reached(db: &ProjectDb, connection: ConnectionId) -> Result<Reached, StoreError> {
  let Some(connection) = db.connection(connection)? else {
    return Ok(Reached::default());
  };
  let endpoint = connection.endpoint().api().map(|api| api.base_url().as_str().into());
  // Another kind of connection is no agent's: dropping the conversion error leaves it out.
  let Ok(agent) = AgentConnection::try_from(connection) else {
    return Ok(Reached::default());
  };
  Ok(Reached {
    provider: Some(agent.provider()),
    endpoint,
    vendor: Some(agent.vendor()),
  })
}
