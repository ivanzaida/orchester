//! Starting a team run from the inputs the team server accepted (team authority §7, protocol 42). The desktop kept them
//! by their operation before it asked ([`ProjectDb::accept_team_inputs`]); the runner reads them back, checked against
//! their hash, and makes sure this computer can run them: the base commit they start from is in the repository, the
//! disk has room, each bound connection and MCP server is here and usable, each tool's secret is bound to one of this
//! project's, and each step's permissions can be enforced. Then it records the run with them in one transaction and
//! starts it. It reads no task or resource definition and asks the server nothing: it holds no server token. The
//! operation has one run, under the key the operation names ([`OperationId::run_key`]): a repeated request answers with
//! it.

use std::{collections::BTreeMap, path::Path};

use orchester_core::{
  id::{OperationId, ResourceId, RunId, SecretId},
  integration::base::RunBaseSource,
  resource::{AgentProfile, StepKind, Tool, Version},
  revision::Revision,
  run::{
    BranchName, NewRun, PreflightInput, PreflightProblem, RunStatus,
    inputs::{BoundReference, InputsOrigin, RunInputs},
    preflight,
  },
  store::{ConnectionStore, McpServerStore, RunStore, SecretStore},
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::{AcceptedInputs, ProjectDb, RunStoreError, StoreError};

use super::{Scheduler, failed, prepare, read_failed};
use crate::git::Git;

impl Scheduler {
  /// Starts the run of the team inputs operation `operation` accepted, or finds the one an earlier request started.
  /// Must be called within a Tokio runtime; returns once the run is recorded as started and its first step is
  /// scheduled.
  pub async fn start_accepted_run(&self, operation: OperationId) -> Result<RunId, Rejection> {
    let _one_at_a_time = self.inner.continuing.lock().await;
    let root = &self.inner.project_root;
    let mut db = self.open()?;
    let read = |error: StoreError| failed("cannot read the accepted inputs", &error);
    if let Some(run) = db.run_of_operation(operation).map_err(read)? {
      let recorded = db
        .run(run)
        .map_err(|error| read_failed("cannot read runs", error))?
        .ok_or(Rejection::RunNotFound)?;
      if recorded.status() == &RunStatus::Pending {
        self.admit(&db)?;
      }
      let task = recorded.task();
      let prepared = prepare::resume_recorded(&mut db, root, task, recorded);
      return self.begin(db, task, prepared);
    }
    let accepted = db
      .accepted_team_inputs(operation)
      .map_err(read)?
      .ok_or(Rejection::InputsNotAccepted)?;
    self.admit(&db)?;
    let task = accepted.task;
    let new = check(&db, root, &accepted)?;
    let prepared = record(&mut db, root, (operation, new), &accepted.inputs);
    self.begin(db, task, prepared)
  }
}

/// The run `accepted` starts, once this computer can run it; every problem that keeps it from starting otherwise.
fn check(db: &ProjectDb, root: &Path, accepted: &AcceptedInputs) -> Result<NewRun, Rejection> {
  let store = |error: StoreError| failed("cannot read the project database", &error);
  let inputs = &accepted.inputs;
  let InputsOrigin::Team(acceptance) = inputs.bundle().origin() else {
    return Err(Rejection::InputsNotAccepted);
  };
  let base = Revision::new(acceptance.base_commit.as_str())
    .map_err(|error| failed("the accepted base commit is not a revision", &error))?;
  let mut problems = preflight_inputs(db, inputs).map_err(store)?;
  if !has_commit(root, &base)? {
    problems.push(PreflightProblem::BaseCommitMissing { commit: base.clone() });
  }
  if !problems.is_empty() {
    return Err(Rejection::Preflight { problems });
  }
  let task = inputs.task(accepted.task, db.project().key(), Timestamp::now());
  let earlier = db
    .runs_of_task(accepted.task)
    .map_err(|error| read_failed("cannot read runs", error))?;
  let snapshot = inputs.bundle().inputs().snapshot().with_sources(
    &db.project_sources().map_err(store)?,
    &db.task_sources(accepted.task).map_err(store)?,
  );
  let limits = inputs.bundle().limits();
  Ok(NewRun {
    task: accepted.task,
    snapshot,
    branch: BranchName::for_next_run(task.key(), earlier.len()),
    base,
    // The branch the run merges into is its module's, which the server assigned.
    base_branch: Some(acceptance.branch.clone()),
    base_source: RunBaseSource::Module {
      module: acceptance.module,
    },
    budget: limits.budget(),
    max_duration: limits.max_duration(),
  })
}

/// What keeps `inputs` from running on this computer: core's preflight of the accepted pipeline, its profiles on their
/// bound connections and MCP servers and its tools, with each tool's secret looked up as the bindings bind it.
fn preflight_inputs(db: &ProjectDb, inputs: &RunInputs) -> Result<Vec<PreflightProblem>, StoreError> {
  let canonical = inputs.bundle().inputs();
  // The preflight reads the bodies only; the versions are dated now.
  let now = Timestamp::now();
  let profiles: BTreeMap<ResourceId, Version<AgentProfile>> = canonical
    .profiles()
    .iter()
    .filter_map(|pinned| {
      let pin = pinned.pinned();
      let body = inputs.profile(pin)?;
      Some((pin.resource, Version::new(pin.version, pin.number, body, now)))
    })
    .collect();
  let tools: BTreeMap<ResourceId, Version<Tool>> = canonical
    .tools()
    .iter()
    .map(|pinned| {
      let pin = pinned.pinned();
      (
        pin.resource,
        Version::new(pin.version, pin.number, pinned.body().clone(), now),
      )
    })
    .collect();
  let secrets = db.secrets()?;
  let mut problems: Vec<PreflightProblem> = preflight(PreflightInput {
    pipeline: Some(canonical.pipeline().body()),
    profiles: &profiles,
    tools: &tools,
    connections: &db.connections()?,
    mcp_servers: &db.mcp_servers()?,
    secrets: &secrets,
  })
  .into_iter()
  .filter(|problem| !matches!(problem, PreflightProblem::SecretMissing { .. }))
  .collect();
  for step in canonical.pipeline().body().steps() {
    let StepKind::Tool { tool: Some(tool) } = &step.kind else {
      continue;
    };
    let named = tools.get(tool).and_then(|version| version.body().secret());
    if let Some(named) = named
      && !secrets.iter().any(|secret| secret.id() == bound_secret(inputs, named))
    {
      problems.push(PreflightProblem::SecretMissing { step: step.key.clone() });
    }
  }
  Ok(problems)
}

/// The secret of this project the bindings bind the publisher's secret `named` to; `named` itself when they bind none.
fn bound_secret(inputs: &RunInputs, named: SecretId) -> SecretId {
  inputs
    .bindings()
    .references
    .iter()
    .find_map(|reference| match reference {
      BoundReference::Secret { named: bound, local } if *bound == named => Some(*local),
      _ => None,
    })
    .unwrap_or(named)
}

/// Whether the repository at `root` has commit `commit`.
fn has_commit(root: &Path, commit: &Revision) -> Result<bool, Rejection> {
  Git::new(root)
    .succeeds(&["cat-file", "-e", &format!("{}^{{commit}}", commit.as_str())])
    .map_err(|error| failed("cannot ask git for the base commit", &error))
}

/// Records the run `new` of `operation`'s `inputs`, with them, then starts it while it is pending.
fn record(
  db: &mut ProjectDb,
  root: &Path,
  (operation, new): (OperationId, NewRun),
  inputs: &RunInputs,
) -> Result<prepare::Prepared, Rejection> {
  let run = db
    .create_run_with_inputs(operation.run_key(), new, inputs, (Timestamp::now(), None))
    .map_err(|error| match error {
      RunStoreError::InputsMismatch { .. } => failed("the accepted inputs do not fit their task", &error),
      other => read_failed("cannot record the run", other),
    })?;
  let resolved = prepare::resolve_task(db, &run)?;
  prepare::finish_recorded(db, root, run, resolved)
}
