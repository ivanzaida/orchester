//! The execution bundle a start's receipt names, read from the server by its hash with every file it pins, each checked
//! against the bundle, and made into the run's inputs with what the start's caller read and chose.

use std::{num::NonZeroU32, time::Duration};

use orchester_api::{
  client::Authority,
  v2::{AuthorityRef, ExecutionBundle, Subject},
};
use orchester_core::{
  blob::ContentHash,
  connection::ConnectionKind,
  run::{
    CommitId,
    inputs::{
      BundleContent, InputsError, InputsOrigin, LocalBindings, LocalConsent, ResourceLabel, RunInputs, TeamAcceptance,
    },
  },
  store::ConnectionStore,
  task::TaskLimits,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;
use serde::{Deserialize, Serialize};

use super::{Answer, TeamWorkError, answer};

/// What the caller of a start read and chose for its run, kept with the start's record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartChoices {
  /// The task's number, from the task read (`GET …/tasks/{task}`). A contract gap: the plan (§7) has the bundle carry
  /// it, and once the server's `ExecutionBundle` does, it comes from there.
  pub task_number: NonZeroU32,
  /// Each pinned resource's slug and name, from the library read. The same contract gap as the number.
  pub labels: Vec<ResourceLabel>,
  /// The local connection each pinned profile runs on here, the person's choice.
  pub bindings: LocalBindings,
  pub consent: LocalConsent,
}

/// What the bundle must say to be the start's: the authority and the base commit it was asked for.
pub(super) struct Expected<'a> {
  pub(super) sha256: &'a ContentHash,
  pub(super) authority: AuthorityRef,
  pub(super) base_commit: &'a CommitId,
}

/// The bundle and the bytes of every file it pins, in its files' order.
pub(super) struct Fetched {
  pub(super) bundle: ExecutionBundle,
  pub(super) files: Vec<Vec<u8>>,
}

/// Reads the bundle `expected` names and every file it pins, each checked; the server's answer when it did not give
/// them.
pub(super) async fn fetch(
  authority: &Authority<'_>,
  expected: &Expected<'_>,
) -> Result<Result<Fetched, Answer>, TeamWorkError> {
  let bundle = match authority.bundle(expected.sha256).await {
    Ok(bundle) => bundle,
    Err(error) => return Ok(Err(answer(error))),
  };
  check(&bundle, expected)?;
  let mut files = Vec::new();
  for file in bundle.inputs().files() {
    let bytes = match authority.bundle_file(expected.sha256, &file.sha256).await {
      Ok(bytes) => bytes,
      Err(error) => return Ok(Err(answer(error))),
    };
    bundle.check_file(&file.sha256, &bytes)?;
    files.push(bytes);
  }
  Ok(Ok(Fetched { bundle, files }))
}

/// Whether `bundle` is the one `expected` names, built for its claim, task and base commit.
fn check(bundle: &ExecutionBundle, expected: &Expected<'_>) -> Result<(), TeamWorkError> {
  let mismatch = |reason| TeamWorkError::BundleMismatch {
    expected: expected.sha256.clone(),
    reason,
  };
  let hash = bundle.sha256().map_err(|_| mismatch("it has no canonical hash"))?;
  let start = bundle.start();
  let checks = [
    (hash == *expected.sha256, "its hash"),
    (start.claim_id == expected.authority.claim_id, "its claim"),
    (start.subject == expected.authority.subject, "its task"),
    (start.base_commit == *expected.base_commit, "its base commit"),
  ];
  match checks.into_iter().find(|(matches, _)| !matches) {
    Some((_, reason)) => Err(mismatch(reason)),
    None => Ok(()),
  }
}

/// The run's inputs of `bundle`, accepted as `acceptance`, with the caller's `choices`. Each pinned profile must be
/// bound to a connection of this project that runs the CLI the bundle binds it to.
pub(super) fn inputs(
  db: &ProjectDb,
  bundle: &ExecutionBundle,
  acceptance: TeamAcceptance,
  choices: StartChoices,
) -> Result<RunInputs, TeamWorkError> {
  check_clis(db, bundle, &choices.bindings)?;
  let start = bundle.start();
  let budget = start
    .limits
    .budget_tokens
    .map(TokenBudget::new)
    .transpose()
    .map_err(|_| InputsError::Unrepresentable("token budget"))?;
  let limits =
    TaskLimits::new(budget, start.limits.max_duration_ms.map(Duration::from_millis)).map_err(InputsError::from)?;
  let content = BundleContent::new(
    InputsOrigin::Team(acceptance),
    (choices.task_number, start.task.clone()),
    choices.labels,
    (limits, start.delivery),
    bundle.inputs().clone(),
  )?;
  Ok(RunInputs::new(content, choices.bindings, choices.consent)?)
}

fn check_clis(db: &ProjectDb, bundle: &ExecutionBundle, bindings: &LocalBindings) -> Result<(), TeamWorkError> {
  let connections = db.connections()?;
  for binding in bundle.bindings() {
    let bound = bindings
      .profiles
      .iter()
      .find(|bound| bound.resource == binding.resource_id)
      .and_then(|bound| {
        connections
          .iter()
          .find(|connection| connection.id() == bound.connection)
      });
    // A profile bound to no connection is the inputs' own refusal; a connection gone is the runner's preflight.
    if let Some(connection) = bound
      && *connection.kind() != ConnectionKind::Agent(binding.cli)
    {
      return Err(TeamWorkError::BindingCli {
        resource: binding.resource_id,
        cli: binding.cli,
      });
    }
  }
  Ok(())
}

/// The subject a start's authority names, which must be a task's.
pub(super) const fn task_subject(subject: Subject) -> Option<orchester_core::id::TaskId> {
  match subject {
    Subject::Task { id } => Some(id),
    Subject::Module { .. } => None,
  }
}
