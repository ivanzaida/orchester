//! What every agent start reads whatever its provider: the agent step and its model, the pinned
//! profile and the connection it runs on, the permissions the agent gets and the extra writable
//! folders they grant. The extra writable folders are resolved in the repository at each start, and
//! those under `.orchester/` that do not exist yet are created: the CLIs ignore a folder that does
//! not exist.

use orchester_core::{
  connection::{AgentConnection, SessionOrigin, invocation::Platform, stream::RunnerProblem},
  id::StepExecutionId,
  permission::{EffectiveApprovalMode, FolderBounds, ManualReason, ResolvedFolder, capability::EnforcedPermissions},
  resource::{AgentProfile, ModelId, Step, StepKey, StepKind},
  run::{ModelChoice, PinnedVersion, StepExecutor},
  store::{ActivityStore, ConnectionStore},
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

  /// Whether `execution`'s agent would start on an account whose CLI last reported in this project that it refuses
  /// turns until a reset that has not come yet (`docs/design/agent-accounts.md`, `XPjSc`): the step does not start
  /// there, the run waits for the reset holding no agent slot, or a person starts the step elsewhere. An account whose
  /// usable credits carry work past its full window starts: the CLI says if it refuses. A step whose connection can't
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
    let now = orchester_core::time::Timestamp::now();
    match self.db.last_account_reading(account) {
      Ok(reading) => reading.is_some_and(|reading| reading.refuses_turns(now) && reading.resets_at(now).is_some()),
      Err(error) => {
        tracing::warn!(%error, "cannot read an account's plan limits; the step starts");
        false
      }
    }
  }

  /// The permissions the agent of step `key` gets on `connection`: the run's for the profile, if they
  /// were changed for it, else the profile's, narrowed by the step, in the approval mode the step runs in there
  /// (Autopilot only where the connection's CLI hands its requests to Orchester, else Manual). What a person on this
  /// computer narrowed the step to in the run's inputs applies when it gives the agents no more than that. Preflight
  /// already refused steps whose permissions the CLI cannot enforce; checked again because an agent never starts with
  /// permissions that are not enforced.
  pub(super) fn enforced(
    &self,
    key: &StepKey,
    body: &AgentProfile,
    connection: &AgentConnection,
  ) -> Result<EnforcedPermissions, RunnerProblem> {
    let permissions = self
      .run
      .step_permissions(key, body.permissions())
      .expect("an agent step of the run has permissions");
    let permissions = match self.resolver.narrowed(key) {
      Some(narrowed) if narrowed.widenings_from(&permissions).is_empty() => narrowed.clone(),
      _ => permissions,
    };
    let effective = EffectiveApprovalMode::of(permissions.approval_mode(), connection.autopilot_support());
    if let EffectiveApprovalMode::Manual {
      why: ManualReason::CantEnforce { why },
    } = &effective
    {
      tracing::info!(step = %key, ?why, "the profile is set to Autopilot, but its CLI cannot hand its requests to Orchester: the step runs as Manual");
    }
    permissions
      .with_effective_mode(&effective)
      .enforced_on(connection.provider(), Platform::current())
      .map_err(|error| problem(error.to_string()))
  }

  /// The model and effort a person chose for this run's step `key` from its Conversation (ORC-322), read at each
  /// turn's start so a change applies from the step's next turn; `None` runs the profile's.
  pub(in super::super) fn chosen_model(&self, key: &StepKey) -> Result<Option<ModelChoice>, RunnerProblem> {
    self
      .db
      .step_model(self.run.id(), key)
      .map_err(|error| problem(error.to_string()))
  }

  /// The model `execution`'s next invocation runs on, as its record keeps it: the run's own for its step if a person
  /// chose one ([`Self::chosen_model`]), else its profile's as the run pinned it.
  pub(in super::super) fn invocation_model(
    &self,
    execution: StepExecutionId,
  ) -> Result<ModelId, crate::execution::step::ExecutionError> {
    let not_agent = crate::execution::step::ExecutionError::NotAnAgentStep(execution);
    let Some(step) = self
      .run
      .execution(execution)
      .and_then(|execution| self.run.snapshot().step(execution.step()))
    else {
      return Err(not_agent);
    };
    let StepExecutor::Agent { model, .. } = step.executor() else {
      return Err(not_agent);
    };
    let chosen = self.db.step_model(self.run.id(), step.key())?;
    Ok(chosen.map_or_else(|| model.clone(), |chosen| chosen.model))
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

  /// The run's own storage, `.orchester/runs/<run>/` (ORC-114), made when it is not there yet: every step reads it, a
  /// step that edits files writes there too.
  pub(super) fn run_storage(&self) -> Result<std::path::PathBuf, RunnerProblem> {
    let storage = orchester_core::workspaces::artifacts_folder(&self.root, self.run.id());
    std::fs::create_dir_all(&storage)
      .map_err(|error| problem(format!("cannot make the run's storage {}: {error}", storage.display())))?;
    Ok(storage)
  }

  /// The extra writable folders `enforced` grants, resolved in the project's repository, the missing
  /// ones created; or why one cannot be given (it was moved, or now lies somewhere refused). An agent that edits
  /// files also gets the run's own storage, where what it writes for people to see stays (ORC-114).
  pub(super) fn writable_folders(&self, enforced: &EnforcedPermissions) -> Result<Vec<ResolvedFolder>, RunnerProblem> {
    let Some(folders) = enforced.permissions().granted_folders() else {
      return Ok(Vec::new());
    };
    let bounds = FolderBounds::new(&self.root, std::env::home_dir().as_deref())
      .map_err(|error| problem(format!("cannot read the repository {}: {error}", self.root.display())))?;
    let mut resolved = folders
      .resolve(&bounds)
      .map_err(|(folder, error)| problem(format!("extra writable folder {folder}: {error}")))?;
    resolved.push(ResolvedFolder::run_storage(&bounds, self.run.id()));
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

  /// The pinned profile version, as the run's inputs bind it, and the agent connection `step` runs it on: the one a
  /// person chose for this run when the step stopped at its account's plan limit, else the profile's own.
  pub(in super::super) fn profile_and_connection(
    &self,
    profile: &PinnedVersion,
    step: &StepKey,
  ) -> Result<(AgentProfile, AgentConnection), RunnerProblem> {
    let body = self
      .resolver
      .profile(&self.db, *profile)
      .map_err(|error| problem(error.to_string()))?;
    let chosen = self
      .db
      .step_connection(self.run.id(), step)
      .map_err(|error| problem(error.to_string()))?;
    let connection = self
      .db
      .connection(chosen.unwrap_or(body.connection()))
      .map_err(|error| problem(error.to_string()))?
      .ok_or_else(|| problem("the agent profile's connection no longer exists".to_owned()))?;
    if let Some(inputs) = self.resolver.team() {
      inputs
        .check_connection(profile.resource, &connection)
        .map_err(|_| problem("the agent profile's connection no longer exists".to_owned()))?;
    }
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
