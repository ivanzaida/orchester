//! What the flow says: what a card runs, where a failure goes, what a gate waits for, and what the end of the line
//! delivers.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::resource::{DeliverMode, Deliverable, MergeMethod, RepositoryAccess};
use orchester_workspace::pipelines::flow::{Conflicts, DeliverCard, Ending, Gate, ReturnsTo, Runs, StepCard};

/// A card's number as the design writes it: `03`.
pub(in crate::screens::pipeline_hub) fn number(number: usize) -> String {
  format!("{number:02}")
}

/// The second line of a card: *Explorer · claude-sonnet-5-5*, *Tool · Checks*, or what is wrong with the step.
pub(super) fn runs(ctx: &Ctx, runs: &Runs) -> Arc<str> {
  match runs {
    Runs::Profile { name, model } => ctx.t_args(
      "pipeline_hub.flow.profileModel",
      [("profile", name.as_ref()), ("model", model.as_ref())],
    ),
    Runs::Tool { name } => ctx.t_args("pipeline_hub.flow.tool", [("tool", name.as_ref())]),
    Runs::NoProfile => ctx.t("pipeline_hub.flow.noProfile"),
    Runs::NoTool => ctx.t("pipeline_hub.flow.noTool"),
    Runs::ProfileNotPublished { .. } => ctx.t("pipeline_hub.flow.profileNotPublished"),
    Runs::ToolNotPublished { .. } => ctx.t("pipeline_hub.flow.toolNotPublished"),
    Runs::ToolNotRunnable { name, .. } => ctx.t_args("pipeline_hub.flow.toolNotRunnable", [("tool", name.as_ref())]),
  }
}

/// What a card offers when its step has nothing to run yet: *Choose profile*, *Choose tool*.
pub(super) fn choose(ctx: &Ctx, runs: &Runs) -> Option<Arc<str>> {
  match runs {
    Runs::NoProfile => Some(ctx.t("pipeline_hub.flow.chooseProfile")),
    Runs::NoTool => Some(ctx.t("pipeline_hub.flow.chooseTool")),
    _ => None,
  }
}

pub(super) fn access(ctx: &Ctx, access: RepositoryAccess) -> Arc<str> {
  ctx.t(match access {
    RepositoryAccess::ReadOnly => "pipeline_hub.flow.readOnly",
    RepositoryAccess::Write => "pipeline_hub.flow.canWrite",
  })
}

/// Where a failure sends the run: an agent step *changes requested → 03*, a tool step *fails → 03*.
pub(super) fn failure(ctx: &Ctx, step: &StepCard, route: &ReturnsTo) -> Arc<str> {
  ctx.t_args(
    if step.is_tool() {
      "pipeline_hub.flow.failureTool"
    } else {
      "pipeline_hub.flow.failure"
    },
    [("number", number(route.number))],
  )
}

/// What a gate waits for: *you approve Plan before 03 starts*.
pub(super) fn gate(ctx: &Ctx, gate: &Gate) -> Arc<str> {
  match gate.before {
    Some(next) => ctx.t_args(
      "pipeline_hub.flow.gateBefore",
      [("step", gate.step.to_string()), ("next", number(next))],
    ),
    None => ctx.t_args("pipeline_hub.flow.gateEnd", [("step", gate.step.to_string())]),
  }
}

/// The Deliver card's second line: *Approve to merge · Squash · conflicts: Builder*.
pub(super) fn deliver(ctx: &Ctx, deliver: &DeliverCard) -> Arc<str> {
  let method = ctx.t(match deliver.method {
    MergeMethod::MergeCommit => "pipeline_hub.flow.mergeCommit",
    MergeMethod::Squash => "pipeline_hub.flow.squash",
  });
  let who = match &deliver.conflicts {
    Conflicts::Profile(name) => name.clone(),
    Conflicts::ProfileNotPublished(name) => {
      ctx.t_args("pipeline_hub.flow.conflictsNotPublished", [("profile", name.as_ref())])
    }
    Conflicts::Person => ctx.t("pipeline_hub.flow.conflictsYou"),
  };
  ctx.t_args(
    match deliver.mode {
      DeliverMode::ApproveToMerge => "pipeline_hub.flow.deliverApprove",
      DeliverMode::AutoMerge => "pipeline_hub.flow.deliverAuto",
    },
    [("method", method.to_string()), ("who", who.to_string())],
  )
}

/// The end of the line and what it says after it: *MERGED INTO THE BASE BRANCH · the task is done*. A pipeline with a
/// Deliver step ends in a merge; the others in what their runs deliver.
pub(super) fn end(ctx: &Ctx, ending: Ending) -> (Arc<str>, Arc<str>) {
  let (label, hint) = match ending {
    Ending::Merged => ("pipeline_hub.flow.endMerged", "pipeline_hub.flow.endMergedHint"),
    Ending::Delivers(Deliverable::Changes) => ("pipeline_hub.flow.endReview", "pipeline_hub.flow.endReviewHint"),
    Ending::Delivers(Deliverable::ChangesOrReport) => (
      "pipeline_hub.flow.endReview",
      "pipeline_hub.flow.endChangesOrReportHint",
    ),
    Ending::Delivers(Deliverable::Report) => ("pipeline_hub.flow.endReport", "pipeline_hub.flow.endReportHint"),
    Ending::Delivers(Deliverable::Outside) => ("pipeline_hub.flow.endOutside", "pipeline_hub.flow.endOutsideHint"),
  };
  (ctx.t(label), ctx.t(hint))
}
