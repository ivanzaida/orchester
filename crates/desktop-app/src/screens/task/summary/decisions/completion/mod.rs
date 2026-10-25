//! The Summary of a task whose latest run passed every step and committed nothing (ORC-93), in place of Ready for
//! review: there is nothing to merge. A run whose result is its report, or work done outside the repository, shows the
//! Ready card with *Complete task* ([`card::ready`]); one whose pipeline expects changes is flagged, *This run committed
//! nothing, but its pipeline expects changes*, with *Complete anyway*, *Return to a step* and *Run again*
//! ([`card::flagged`]); a completed run says who completed it and when ([`card::completed`]). Under the card the run's
//! report, the MCP tool calls of work done outside the repository ([`report`]), its stages and *Runs of this task*;
//! beside them *Run details*. The design has no frames of its own for these states: they follow the Ready card
//! (`hyzq2` in `F2LtFn`), the warning notice (`iw9rS`) and the Summary's sections. Completing runs off the UI thread;
//! the task is read again after it.

mod card;
mod report;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  components::Column,
  core::Signal,
  node::Element,
};
use orchester_core::{id::RunId, resource::WithoutCommits, run::CompletedBy, time::Timestamp};
use orchester_workspace::{
  background,
  messages::Message,
  tasks::{completion::complete_in, delivery::Delivery},
};

use super::{
  super::run_details::run_details, TaskContext, arranged, return_ended::return_card, runs_section, stages::stages,
};
use crate::{mcp::input_gate::InputGate, services::Services};

/// Whether the Summary of `task` is that of a latest run that passed every step and committed nothing.
pub fn shown(task: &TaskContext) -> bool {
  task.detail.delivery == Delivery::NothingToMerge && task.detail.merge.is_none() && task.detail.result.is_some()
}

/// A completion under way, for its button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Completing;

impl DevtoolsInspectable for Completing {}

/// Completes a run, anyway when its pipeline expects changes: off the UI thread, then the task is read again.
pub type CompleteAction = FutureAction<(RunId, bool), Completing, Message>;

pub struct CompleteRun {
  /// Window input pressed *Complete anyway* while the person does not let automation decide for them.
  person_only: Signal<bool>,
  gate: Option<InputGate>,
}

impl Component for CompleteRun {
  type Props = TaskContext;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      person_only: ctx.signal(false),
      gate: ctx.use_context::<InputGate>(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TaskContext>().clone();
    let action = complete_action(ctx, &props);
    let (Some(run), Some(result)) = (props.detail.run.as_ref(), props.detail.result.as_ref()) else {
      return Column::new().into();
    };
    let now = Services::of(ctx).clock.now();
    let top = match (&result.completion, result.without_commits()) {
      (Some(completion), _) => card::completed(ctx, run, completion),
      (None, WithoutCommits::Completes) => card::ready(ctx, (&props, run, result), &action),
      (None, WithoutCommits::Flagged) => {
        let gated = card::Gated {
          gate: self.gate.as_ref(),
          person_only: &self.person_only,
        };
        card::flagged(ctx, (&props, run), &action, &gated)
      }
    };
    let parts: Vec<Element> = std::iter::once(top)
      .chain(report::sections(ctx, run, result))
      .chain([stages(run, None, now)])
      .chain(runs_section(ctx, &props))
      .collect();
    let linked = crate::screens::task::sharing::linked(ctx, &props);
    let section = crate::screens::task::sharing::section(ctx, &props, (linked.as_ref(), None));
    let lead = return_card(ctx, &props);
    let side = run_details(ctx, &props, run, (lead, section));
    arranged(ctx, parts, None, Some(side))
  }
}

/// Completes runs of `task`'s project as the person pressed, then reads the project's tasks again.
fn complete_action(ctx: &mut Ctx, task: &TaskContext) -> CompleteAction {
  let services = Services::of(ctx);
  let (root, programs, feed) = (
    task.project.open.repo_root.clone(),
    services.programs,
    task.project.feed.clone(),
  );
  ctx.future_action(move |(run, anyway): (RunId, bool)| {
    let (root, programs, feed) = (root.clone(), programs.clone(), feed.clone());
    async move {
      let completed =
        background::run(move || complete_in((&root, &programs), run, (CompletedBy::Person, anyway), Timestamp::now()))
          .await?;
      feed.saved();
      completed.map(|_| Completing).inspect_err(|message| {
        tracing::warn!(key = message.key, %run, "the run was not completed");
      })
    }
  })
}
