//! What every agent start reads whatever its provider: the agent step and its model, the pinned
//! profile and the connection it runs on, the permissions the agent gets and the extra writable
//! folders they grant. The extra writable folders are resolved in the repository at each start, and
//! those under `.orchester/` that do not exist yet are created: the CLIs ignore a folder that does
//! not exist.

use std::path::PathBuf;

use orchester_core::{
  connection::{AgentConnection, AgentProvider, invocation::Platform, stream::RunnerProblem},
  id::StepExecutionId,
  permission::{FolderBounds, capability::EnforcedPermissions},
  resource::{AgentProfile, ModelId, Step, StepKey, StepKind},
  run::{PinnedVersion, StepExecutor},
  store::{ConnectionStore, ResourceStore},
};

use super::{super::super::run::RunTask, problem};

impl RunTask {
  /// The permissions the agent of step `key` gets on `provider`: the run's for the profile, if they
  /// were changed for it, else the profile's, narrowed by the step. Preflight already refused steps
  /// whose permissions the CLI cannot enforce; checked again because an agent never starts with
  /// permissions that are not enforced.
  pub(super) fn enforced(
    &self,
    key: &StepKey,
    body: &AgentProfile,
    provider: AgentProvider,
  ) -> Result<EnforcedPermissions, RunnerProblem> {
    self
      .run
      .step_permissions(key, body.permissions())
      .expect("an agent step of the run has permissions")
      .enforced_on(provider, Platform::current())
      .map_err(|error| problem(error.to_string()))
  }

  /// The agent step `execution` executes, the profile version that runs it and its model.
  pub(super) fn agent_step(
    &self,
    execution: StepExecutionId,
  ) -> Result<(&Step, &PinnedVersion, &ModelId), RunnerProblem> {
    let key = self
      .run
      .execution(execution)
      .expect("the execution's model was found")
      .step();
    let step = self
      .run
      .snapshot()
      .body()
      .step(key)
      .expect("executions name snapshot steps");
    let (StepKind::Agent { .. }, Some(StepExecutor::Agent { profile, model, .. })) =
      (&step.kind, self.run.snapshot().step(key).map(|step| step.executor()))
    else {
      return Err(problem(format!("step `{key}` is not an agent step")));
    };
    Ok((step, profile, model))
  }

  /// The extra writable folders `enforced` grants, resolved in the project's repository, the missing
  /// ones created; or why one cannot be given (it was moved, or now lies somewhere refused).
  pub(super) fn writable_roots(&self, enforced: &EnforcedPermissions) -> Result<Vec<PathBuf>, RunnerProblem> {
    let Some(folders) = enforced
      .permissions()
      .granted_folders()
      .filter(|folders| !folders.is_empty())
    else {
      return Ok(Vec::new());
    };
    let bounds = FolderBounds::new(&self.root, std::env::home_dir().as_deref())
      .map_err(|error| problem(format!("cannot read the repository {}: {error}", self.root.display())))?;
    let resolved = folders
      .resolve(&bounds)
      .map_err(|(folder, error)| problem(format!("extra writable folder {folder}: {error}")))?;
    resolved
      .into_iter()
      .map(|folder| {
        if folder.is_missing() {
          std::fs::create_dir_all(folder.path()).map_err(|error| {
            problem(format!(
              "cannot create the extra writable folder {}: {error}",
              folder.path().display()
            ))
          })?;
        }
        Ok(folder.path().to_path_buf())
      })
      .collect()
  }

  /// The pinned profile version and the agent connection it runs on.
  pub(in super::super) fn profile_and_connection(
    &self,
    profile: &PinnedVersion,
  ) -> Result<(AgentProfile, AgentConnection), RunnerProblem> {
    let body: AgentProfile = self
      .db
      .version::<AgentProfile>(profile.resource, profile.number)
      .map_err(|error| problem(error.to_string()))?
      .ok_or_else(|| problem(format!("agent profile version {} is missing", profile.number)))?
      .body()
      .clone();
    let connection = self
      .db
      .connection(body.connection())
      .map_err(|error| problem(error.to_string()))?
      .ok_or_else(|| problem("the agent profile's connection no longer exists".to_owned()))?;
    let connection =
      AgentConnection::try_from(connection).map_err(|_| problem("the connection is not an agent CLI".to_owned()))?;
    Ok((body, connection))
  }
}
