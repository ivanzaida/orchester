//! Tool requests awaiting a person's decision, with the CLI of the agent that asks: its approval
//! dialog says what an approval means for that CLI (Codex keeps an approved command in its
//! sandbox; Claude Code has none).

use orchester_core::{
  connection::{AgentProvider, ConnectionKind},
  resource::AgentProfile,
  run::{PendingToolApproval, StepExecutor},
  store::{ConnectionStore, ResourceStore, RunStore},
};
use orchester_store::{ProjectDb, StoreError};

/// A pending request and the CLI of its agent; `None` when that cannot be read any more.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShownApproval {
  pub approval: PendingToolApproval,
  pub provider: Option<AgentProvider>,
}

/// The project's pending requests, oldest first, each with its agent's CLI.
pub fn pending(db: &ProjectDb) -> Result<Vec<ShownApproval>, StoreError> {
  Ok(
    db.pending_tool_approvals_all()?
      .into_iter()
      .map(|approval| {
        let provider = provider(db, &approval);
        ShownApproval { approval, provider }
      })
      .collect(),
  )
}

/// The CLI the request's step ran on: its run's pinned profile version and that profile's
/// connection. A missing or unreadable part leaves it unknown, which the dialog shows with its
/// general wording.
fn provider(db: &ProjectDb, approval: &PendingToolApproval) -> Option<AgentProvider> {
  // `.ok()`: an unreadable run, profile or connection only loses the provider-specific wording;
  // the request itself stays shown and decidable, and the list reports its own read errors.
  let run = db.run(approval.run).ok()??;
  let step = run.execution(approval.execution)?.step().clone();
  let StepExecutor::Agent { profile, .. } = run.snapshot().step(&step)?.executor() else {
    return None;
  };
  let version = db.version::<AgentProfile>(profile.resource, profile.number).ok()??;
  let connection = db.connection(version.body().connection()).ok()??;
  match connection.kind() {
    ConnectionKind::Agent(provider) => Some(*provider),
    ConnectionKind::GitHub { .. } => None,
  }
}
