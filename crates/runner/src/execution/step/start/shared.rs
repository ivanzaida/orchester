//! What every agent start reads whatever its provider: the agent step and its model, the pinned
//! profile and the connection it runs on, the permissions the agent gets and the extra writable
//! folders they grant. The extra writable folders are resolved in the repository at each start, and
//! those under `.orchester/` that do not exist yet are created: the CLIs ignore a folder that does
//! not exist.

use orchester_core::{
  connection::{AgentConnection, AgentProvider, SessionOrigin, invocation::Platform, stream::RunnerProblem},
  id::StepExecutionId,
  permission::{FolderBounds, ResolvedFolder, capability::EnforcedPermissions},
  resource::{AgentProfile, ModelId, Step, StepKey, StepKind},
  run::{PinnedVersion, StepExecutor},
  store::{ActivityStore, ConnectionStore, ResourceStore},
};

use super::{super::super::run::RunTask, problem};

impl RunTask {
  /// A step whose agent never started waits for its account's reset before it takes an agent slot: it opens, and
  /// the run stops at it once nothing else works (`settle`); `true` then. A step stopped at the limit tries again
  /// when asked: the CLI says.
  pub(in super::super) fn waits_for_plan_reset(
    &mut self,
    execution: StepExecutionId,
  ) -> Result<bool, crate::execution::step::ExecutionError> {
    // A person asked to try it now: it starts once (the ask is used up when its agent takes a slot, `start`); the CLI
    // says whether the limit still holds.
    if self.try_at_plan_limit == Some(execution) {
      return Ok(false);
    }
    if self.last_invocation(execution)?.is_some() || !self.account_at_plan_limit(execution) {
      return Ok(false);
    }
    let pending = self.run.execution(execution).map(|execution| execution.status())
      == Some(orchester_core::run::StepStatus::Pending);
    if pending {
      self.transition(|run, now| run.start_step(execution, now))?;
    }
    self
      .pending
      .limit
      .get_or_insert(orchester_core::run::RunLimit::PlanLimit { execution });
    Ok(true)
  }

  /// Whether `execution`'s agent would start on an account whose plan limit its CLI last reported reached in this
  /// project and not reset yet (`docs/design/agent-accounts.md`, `XPjSc`): the step does not start there, the run
  /// waits for the reset holding no agent slot, or a person starts the step elsewhere. A step whose connection can't
  /// be read starts and says why.
  fn account_at_plan_limit(&self, execution: StepExecutionId) -> bool {
    let Ok((step, profile, _)) = self.agent_step(execution) else {
      return false;
    };
    let Ok((_, connection)) = self.profile_and_connection(profile, &step.key) else {
      return false;
    };
    let Some(account) = connection.plan_account() else {
      return false;
    };
    match self.db.last_account_reading(account) {
      Ok(reading) => reading.is_some_and(|reading| reading.resets_at(orchester_core::time::Timestamp::now()).is_some()),
      Err(error) => {
        tracing::warn!(%error, "cannot read an account's plan limits; the step starts");
        false
      }
    }
  }

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
  pub(in super::super) fn agent_step(
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
  pub(super) fn writable_folders(&self, enforced: &EnforcedPermissions) -> Result<Vec<ResolvedFolder>, RunnerProblem> {
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
        Ok(folder)
      })
      .collect()
  }

  /// The pinned profile version and the agent connection `step` runs it on: the one a person chose for this run
  /// when the step stopped at its account's plan limit, else the profile's own.
  pub(in super::super) fn profile_and_connection(
    &self,
    profile: &PinnedVersion,
    step: &StepKey,
  ) -> Result<(AgentProfile, AgentConnection), RunnerProblem> {
    let body: AgentProfile = self
      .db
      .version::<AgentProfile>(profile.resource, profile.number)
      .map_err(|error| problem(error.to_string()))?
      .ok_or_else(|| problem(format!("agent profile version {} is missing", profile.number)))?
      .body()
      .clone();
    let chosen = self
      .db
      .step_connection(self.run.id(), step)
      .map_err(|error| problem(error.to_string()))?;
    let connection = self
      .db
      .connection(chosen.unwrap_or(body.connection()))
      .map_err(|error| problem(error.to_string()))?
      .ok_or_else(|| problem("the agent profile's connection no longer exists".to_owned()))?;
    let connection =
      AgentConnection::try_from(connection).map_err(|_| problem("the connection is not an agent CLI".to_owned()))?;
    Ok((body, connection))
  }
}

impl RunTask {
  /// Forgets the CLI session `continuation` resumes when `execution`'s agent now starts on another account than the
  /// session's: a session lives in its account's folder, so the step starts a new session there from the checkpoint
  /// (`docs/design/agent-accounts.md`, a step continued on another account after a plan limit).
  pub(in super::super) fn keep_session_on_its_account(
    &self,
    execution: StepExecutionId,
    continuation: &mut super::super::Continuation,
  ) -> Result<(), orchester_store::StoreError> {
    let Some(session) = continuation.session.as_deref() else {
      return Ok(());
    };
    let Ok((step, profile, _)) = self.agent_step(execution) else {
      return Ok(());
    };
    let Ok((_, connection)) = self.profile_and_connection(profile, &step.key) else {
      return Ok(());
    };
    let invocations = self.db.invocations(self.run.id())?;
    let Some(last) = invocations
      .iter()
      .rev()
      .find(|invocation| invocation.execution == execution && invocation.session.as_deref() == Some(session))
    else {
      return Ok(());
    };
    // A native agent's conversation is Orchester's own, kept whatever the connection runs on.
    if connection.provider().is_native() {
      return Ok(());
    }
    let account = self
      .db
      .invocation_accounts(self.run.id())?
      .into_iter()
      .find(|(invocation, _)| *invocation == last.id)
      .map(|(_, account)| account);
    let provider = self.db.invocation_provider(last.id)?;
    let made = SessionOrigin { account, provider };
    if !made.continues_on(&SessionOrigin::of(&connection)) {
      continuation.session = None;
    }
    Ok(())
  }
}
