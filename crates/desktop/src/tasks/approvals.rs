//! Tool requests awaiting a person's decision, with the CLI of the agent that asks and whether its
//! step only reads: the approval dialog says what an approval means for that CLI (an approved
//! Codex command runs outside its sandbox, Claude Code has none) and that it can change files in a
//! step that only reads.

use orchester_core::{
  connection::{AgentProvider, ConnectionKind},
  permission::RepositoryAccess,
  resource::{AgentProfile, ModelId},
  run::{PendingToolApproval, PinnedVersion, StepExecutor},
  store::{ActivityStore, ConnectionStore, ResourceStore, RunStore},
};
use orchester_store::{ProjectDb, StoreError};

/// A pending request and the CLI of its agent; `None` when that cannot be read any more.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShownApproval {
  pub approval: PendingToolApproval,
  pub provider: Option<AgentProvider>,
  /// The model the asking step runs on, by the run's pinned snapshot.
  pub model: Option<ModelId>,
  /// The asking invocation launched read-only, by its recorded launch permissions.
  pub read_only: bool,
}

/// The project's pending requests, oldest first, each with its agent's CLI.
pub fn pending(db: &ProjectDb) -> Result<Vec<ShownApproval>, StoreError> {
  Ok(
    db.pending_tool_approvals_all()?
      .into_iter()
      .map(|approval| {
        let (provider, model) = engine(db, &approval).map_or((None, None), |(provider, model)| (provider, Some(model)));
        let read_only = launched_read_only(db, &approval);
        ShownApproval {
          approval,
          provider,
          model,
          read_only,
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
fn engine(db: &ProjectDb, approval: &PendingToolApproval) -> Option<(Option<AgentProvider>, ModelId)> {
  // `.ok()`: an unreadable run, profile or connection only loses the provider-specific wording;
  // the request itself stays shown and decidable, and the list reports its own read errors.
  let run = db.run(approval.run).ok()??;
  let step = run.execution(approval.execution)?.step().clone();
  let StepExecutor::Agent { profile, model, .. } = run.snapshot().step(&step)?.executor() else {
    return None;
  };
  Some((provider(db, *profile), model.clone()))
}

/// The CLI of pinned profile version `profile`'s connection, while both can be read.
fn provider(db: &ProjectDb, profile: PinnedVersion) -> Option<AgentProvider> {
  // `.ok()`: as above, an unreadable profile or connection only loses the provider-specific wording.
  let version = db.version::<AgentProfile>(profile.resource, profile.number).ok()??;
  let connection = db.connection(version.body().connection()).ok()??;
  match connection.kind() {
    ConnectionKind::Agent(provider) => Some(*provider),
    ConnectionKind::GitHub { .. } => None,
  }
}
