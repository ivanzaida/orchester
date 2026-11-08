//! The status line while the person's last Run was refused because the task's pipeline is not ready on this computer
//! (`Status line` `g9ufW` in R1 `Ku0qu` for a team project, in L9 `f5CEX` for a local one): the task's state and why in
//! the warning tone (`Not started · not ready on this computer`), then in `text_muted` when Run was pressed and, when
//! the task has an earlier run that ended, when it ended (`Run pressed 15:40 · run 8 ended 15:21`). It reads as the
//! Summary's card does, by the same re-read of what is missing ([`not_ready::missing_now`]), so the line is the
//! ordinary one again once the card goes.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  node::Element,
};
use orchester_core::time::Timestamp;
use orchester_workspace::display;

use super::run_line;
use crate::screens::task::{
  TaskContext,
  not_ready::{self, CardProps},
};

#[derive(Clone, PartialEq)]
pub struct LineProps {
  card: CardProps,
  /// When the refused Run was pressed.
  pressed: Timestamp,
  /// The ordinary line says the run's approval mode.
  mode: bool,
}

impl DevtoolsInspectable for LineProps {}

/// The line over the task's tabs when its last start was refused for not being ready here, until what it lists is
/// found complete; `None` when the ordinary line applies.
pub fn shown(ctx: &mut Ctx, task: &TaskContext, mode: bool) -> Option<Element> {
  let refusal = not_ready::refused(ctx, task)?;
  let pressed = not_ready::pressed(ctx, task)?;
  Some(ctx.mount_keyed::<NotReadyLine>(
    "task-not-ready-line",
    LineProps {
      card: CardProps {
        task: task.clone(),
        refusal,
      },
      pressed,
      mode,
    },
  ))
}

pub struct NotReadyLine;

impl Component for NotReadyLine {
  type Props = LineProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<LineProps>().clone();
    let task = &props.card.task;
    if not_ready::missing_now(ctx, &props.card).is_none() {
      return run_line::status_line(ctx, task, props.mode);
    }
    let words = if task.team.is_some() {
      ctx.t("task.status.notReadyHere")
    } else {
      ctx.t("task.status.notReady")
    };
    let line = line(ctx, task, props.pressed);
    run_line::not_ready(ctx, task, (words, &line))
  }
}

/// `Run pressed 15:40`, and `run 8 ended 15:21` after it when the task's latest run ended.
fn line(ctx: &Ctx, task: &TaskContext, pressed: Timestamp) -> Arc<str> {
  let pressed = display::clock(pressed);
  let ended = task
    .detail
    .run
    .as_ref()
    .and_then(|run| run.finished_at.map(|at| (run.run.number(), at)));
  match ended {
    Some((number, at)) => ctx.t_args(
      "task.runLine.notReadyAfterRun",
      [
        ("pressed", pressed),
        ("run", number.get().to_string()),
        ("ended", display::clock(at)),
      ],
    ),
    None => ctx.t_args("task.runLine.notReadyPressed", [("pressed", pressed)]),
  }
}
