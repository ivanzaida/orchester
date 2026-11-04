//! A finished step's todo list, folded into its divider (09 d `u5bz4S`): the chip *Plan · 7 of 7* at the divider's
//! end, which opens the list as the step left it under the divider, read only (09 e `latq2`). A step that stopped
//! with items left — it failed, or the run stopped or was interrupted while it worked — says under them how many it
//! left and when (09 f `rj5q6`). The chip is the step's latest execution's, while its list is not pinned over the
//! composer (`super::super::todo`); a step whose agent kept no list gets none (07b `r44wqm`).

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::run::StepStatus;
use orchester_workspace::{display, tasks::timeline::StepHead};

use super::Timeline;
use crate::ui::todo_list::{chip, ended, left_undone};

/// The chip of the step `head` names, and its list under the divider while the person opened it; `None` for a step
/// with no list, or whose list is pinned over the composer.
pub(super) fn of_step(ctx: &Ctx, timeline: &Timeline, head: &StepHead) -> Option<(Element, Option<Element>)> {
  let execution = head.execution?;
  let run = timeline.detail.run.as_ref()?;
  let step = run.steps.iter().find(|step| step.execution == Some(execution))?;
  let list = step.todo.as_ref()?;
  if timeline.pins && super::super::todo::works(step) {
    return None;
  }
  let key: Arc<str> = format!("plan-{execution}").into();
  let open = timeline.toggles.is_open(&key, false);
  let id = format!("conversation-plan-chip-{execution}");
  let toggles = timeline.toggles.clone();
  let toggle = move || toggles.flip(&key, false);
  let chip = chip(ctx, &id, (list, open), toggle);
  let opened = open.then(|| {
    // When it stopped: as its agent's last turn ended, else as the run ended or paused.
    let at = head.ended.or(run.finished_at).or(run.paused_at);
    let left = left_undone(list, step.status == Some(StepStatus::Passed));
    let note = left.zip(at).map(|(left, at)| {
      ctx.t_args(
        "conversation.plan.notDone",
        [
          ("count", left.to_string().as_str()),
          ("step", &*step.name),
          ("time", display::clock(at).as_str()),
        ],
      )
    });
    ended(&format!("conversation-plan-list-{execution}"), list, note.as_deref())
  });
  Some((chip, opened))
}
