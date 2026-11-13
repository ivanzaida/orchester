//! What the pipeline screens say about a step, a version or a change, as messages.

use orchester_core::{
  resource::{DeliverChange, Deliverable, PipelineChange, Step, StepChange, StepKind},
  tokens::compact_tokens,
};

use super::data::Assignables;
use crate::messages::Message;

/// Second line of a step card: who runs it (`K4DMy` in `ajikA`).
pub fn executor_line(step: &Step, assignables: &Assignables) -> Message {
  match &step.kind {
    StepKind::Agent { profile: None, .. } => Message::new("pipelines.step.noProfile"),
    StepKind::Tool { tool: None } => Message::new("pipelines.step.noTool"),
    StepKind::Agent {
      profile: Some(id),
      model,
      ..
    } => match assignables.profile(*id) {
      Some(profile) => Message::new(match profile.connection {
        Some(_) => "pipelines.step.profileConnectionModel",
        None => "pipelines.step.profileModel",
      })
      .with("profile", profile.name.to_string())
      .with(
        "connection",
        profile.connection.as_deref().unwrap_or_default().to_owned(),
      )
      .with("model", model.as_ref().unwrap_or(&profile.model).as_str().to_owned()),
      None => Message::new("pipelines.step.unpublishedProfile"),
    },
    StepKind::Tool { tool: Some(id) } => match assignables.tool(*id) {
      Some(tool) => Message::new("pipelines.step.tool").with("tool", tool.name.to_string()),
      None => Message::new("pipelines.step.unpublishedTool"),
    },
  }
}

/// "6 steps", "1 step".
pub fn step_count(count: usize) -> Message {
  if count == 1 {
    Message::new("pipelines.count.step.one")
  } else {
    Message::new("pipelines.count.step.other").with("count", count.to_string())
  }
}

/// "No runs", "1 run", "3 runs" that pinned a version.
pub fn run_count(count: usize) -> Message {
  match count {
    0 => Message::new("pipelines.count.run.none"),
    1 => Message::new("pipelines.count.run.one"),
    _ => Message::new("pipelines.count.run.other").with("count", count.to_string()),
  }
}

/// One line of the change list in the publish confirmation (`LXUS6`) and the version history.
pub fn change(change: &PipelineChange) -> Message {
  let (name, change) = match change {
    PipelineChange::Step { name, change, .. } => (name, change),
    PipelineChange::Description => return Message::new("pipelines.changes.description"),
    PipelineChange::Deliver(DeliverChange::Added) => return Message::new("pipelines.changes.deliverAdded"),
    PipelineChange::Deliver(DeliverChange::Removed) => return Message::new("pipelines.changes.deliverRemoved"),
    PipelineChange::Deliver(DeliverChange::Changed) => return Message::new("pipelines.changes.deliverChanged"),
    PipelineChange::Delivers { to, .. } => return Message::new(delivers_change_key(*to)),
  };
  let about = |key: &'static str| Message::new(key).with("step", name.as_str().to_owned());
  match change {
    StepChange::Added => about("pipelines.changes.added"),
    StepChange::Removed => about("pipelines.changes.removed"),
    StepChange::Renamed { from } => about("pipelines.changes.renamed").with("from", from.as_str().to_owned()),
    StepChange::Executor => about("pipelines.changes.executor"),
    StepChange::Model => about("pipelines.changes.model"),
    StepChange::RepositoryAccess => about("pipelines.changes.repositoryAccess"),
    StepChange::Permissions => about("pipelines.changes.permissions"),
    StepChange::Instructions => about("pipelines.changes.instructions"),
    StepChange::Dependencies => about("pipelines.changes.dependencies"),
    StepChange::ApprovalGate { now: true } => about("pipelines.changes.approvalAdded"),
    StepChange::ApprovalGate { now: false } => about("pipelines.changes.approvalRemoved"),
    StepChange::FailureRoute => about("pipelines.changes.failureRoute"),
    StepChange::MaxRetries { from, to } => about("pipelines.changes.maxRetries")
      .with("from", from.to_string())
      .with("to", to.to_string()),
    StepChange::Budget { from, to } => about("pipelines.changes.budget")
      .with("from", budget_text(*from))
      .with("to", budget_text(*to)),
  }
}

/// A step budget in the compact form (`400k`); the message adds "tokens".
fn budget_text(budget: Option<orchester_core::tokens::TokenBudget>) -> String {
  budget.map_or_else(|| "—".to_owned(), |budget| compact_tokens(budget.get()))
}

/// "Runs now deliver a report": the change to what the pipeline's runs deliver, by what they deliver now.
const fn delivers_change_key(to: Deliverable) -> &'static str {
  match to {
    Deliverable::Changes => "pipelines.changes.delivers.changes",
    Deliverable::Report => "pipelines.changes.delivers.report",
    Deliverable::Outside => "pipelines.changes.delivers.outside",
    Deliverable::ChangesOrReport => "pipelines.changes.delivers.changesOrReport",
  }
}
