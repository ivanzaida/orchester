//! Tool requests awaiting a person's decision, with the CLI of the agent that asks and whether its
//! step only reads: the approval dialog says what an approval means for that CLI (an approved
//! Codex command runs outside its sandbox, Claude Code has none) and that it can change files in a
//! step that only reads. Each names its task: on that task's Conversation tab the request is a card
//! of the timeline, and the dialog does not open.

use orchester_core::{
  connection::{AgentConnection, AgentProvider, EngineVendor},
  id::TaskId,
  permission::RepositoryAccess,
  resource::{AgentProfile, ModelId},
  run::{PendingToolApproval, PinnedVersion, StepExecutor},
  store::{ActivityStore, ConnectionStore, ResourceStore, RunStore},
};
use orchester_store::{ProjectDb, StoreError};

use crate::navigation::route::{Page, TaskPageTab};

/// A pending request and the CLI of its agent; `None` when that cannot be read any more.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShownApproval {
  pub approval: PendingToolApproval,
  pub provider: Option<AgentProvider>,
  /// Who receives the agent's prompts, as its engine is named.
  pub vendor: Option<EngineVendor>,
  /// The model the asking step runs on, by the run's pinned snapshot.
  pub model: Option<ModelId>,
  /// The asking invocation launched read-only, by its recorded launch permissions.
  pub read_only: bool,
  /// The task of the run that asks; `None` when its run cannot be read.
  pub task: Option<TaskId>,
}

impl ShownApproval {
  /// Whether the dialog opens for it on `page`: everywhere but its task's Conversation tab, which
  /// shows it inline.
  pub fn opens_on(&self, page: Option<Page>) -> bool {
    !matches!((page, self.task), (Some(Page::Task(shown, TaskPageTab::Conversation)), Some(task)) if shown == task)
  }
}

/// The project's pending requests, oldest first, each with its agent's CLI.
pub fn pending(db: &ProjectDb) -> Result<Vec<ShownApproval>, StoreError> {
  Ok(
    db.pending_tool_approvals_all()?
      .into_iter()
      .map(|approval| {
        let (connection, model) =
          engine(db, &approval).map_or((None, None), |(connection, model)| (connection, Some(model)));
        let read_only = launched_read_only(db, &approval);
        // `.ok()`: as for the provider, an unreadable run only loses where the request shows inline.
        let task = db.run(approval.run).ok().flatten().map(|run| run.task());
        ShownApproval {
          approval,
          provider: connection.as_ref().map(AgentConnection::provider),
          vendor: connection.as_ref().map(AgentConnection::vendor),
          model,
          read_only,
          task,
        }
      })
      .collect(),
  )
}

/// Whether the invocation that asks launched with read-only repository access. An unreadable or
/// unrecorded launch reads as not read-only, which only loses the read-only wording.
fn launched_read_only(db: &ProjectDb, approval: &PendingToolApproval) -> bool {
  // `.ok()`: as for the provider, the request stays shown and decidable without the wording.
  db.invocations(approval.run)
    .ok()
    .into_iter()
    .flatten()
    .find(|invocation| invocation.id == approval.invocation)
    .and_then(|invocation| invocation.permissions)
    .is_some_and(|launch| launch.permissions().repository() == RepositoryAccess::ReadOnly)
}

/// The CLI the request's step ran on, through its run's pinned profile version and that profile's
/// connection, and the step's pinned model. A missing or unreadable part leaves it unknown, which the
/// dialog shows with its general wording.
fn engine(db: &ProjectDb, approval: &PendingToolApproval) -> Option<(Option<AgentConnection>, ModelId)> {
  // `.ok()`: an unreadable run, profile or connection only loses the provider-specific wording;
  // the request itself stays shown and decidable, and the list reports its own read errors.
  let run = db.run(approval.run).ok()??;
  let step = run.execution(approval.execution)?.step().clone();
  let StepExecutor::Agent { profile, model, .. } = run.snapshot().step(&step)?.executor() else {
    return None;
  };
  Some((connection(db, *profile), model.clone()))
}

/// The agent connection of pinned profile version `profile`, while both can be read.
fn connection(db: &ProjectDb, profile: PinnedVersion) -> Option<AgentConnection> {
  // `.ok()`: as above, an unreadable profile or connection only loses the provider-specific wording;
  // a connection of another kind is no agent's.
  let version = db.version::<AgentProfile>(profile.resource, profile.number).ok()??;
  let connection = db.connection(version.body().connection()).ok()??;
  AgentConnection::try_from(connection).ok()
}
