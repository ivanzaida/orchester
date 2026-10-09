//! Requests that wait for the person, and runs that answer their own (handover §3.3): *3 approvals
//! waiting · Builder asks to run cargo add serde · KON-42* with *Review*, which opens the oldest request's
//! card in its task's Conversation (no dialog in this build, decision D4); *Auto-approving · Run 142 ·
//! KON-42* (or how many runs) with *Stop*, which stops each of them. When the bar is narrow, what the
//! request asks trims its end and then drops out, the run drops out, and later the words after the
//! count; the hand, the count, *Review* and *Stop* stay.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  layout::layout_kind::ShrinkLimit,
  node::Element,
};
use orchester_core::project::TaskKey;
use orchester_workspace::tasks::{approvals::ShownApproval, data::AutoApprovingRun};

use super::{
  facts::{BarAction, Dispatch, ProjectFacts},
  item::{action, dropping, give_way, glyph, item, label, trimmed},
};
use crate::{
  routes::{ProjectPage, Route, TaskView},
  ui::icon::Icon,
};

/// What the request asks keeps at least this much room before it drops out.
const ASKS_MIN_WIDTH: f32 = 100.0;

/// The waiting requests, oldest first; nothing when none waits.
pub fn approvals(ctx: &Ctx, project: &ProjectFacts, pending: &[ShownApproval], dispatch: &Dispatch) -> Vec<Element> {
  let Some(oldest) = pending.first() else {
    return Vec::new();
  };
  let count = pending.len();
  let words = ctx.t(if count == 1 {
    "shell.status.approvals.waiting.one"
  } else {
    "shell.status.approvals.waiting.other"
  });
  let start = item("status-approvals")
    .describe("count", count.to_string())
    .flex_shrink(1.0)
    .shrink_order(give_way::APPROVAL_WORDS)
    .shrink_limit(ShrinkLimit::Content)
    .child(glyph(Icon::Hand, PaletteColor::Warning))
    .child(label(&count.to_string(), PaletteColor::Warning))
    .child(dropping(&words, PaletteColor::Warning));
  let asks = item("status-approvals-asks")
    .flex_shrink(1.0)
    .shrink_order(give_way::ASKS)
    .shrink_drop_below(ASKS_MIN_WIDTH)
    .child(trimmed(&asks(ctx, oldest), PaletteColor::TextSecondary).id("status-approvals-asks-text"));
  let mut items = vec![start.into(), asks.into()];
  let key = oldest.task.as_deref().and_then(|key| key.parse::<TaskKey>().ok());
  if let Some(key) = key {
    let route = Route::Project(project.id, ProjectPage::Task(key, TaskView::Conversation));
    let dispatch = dispatch.clone();
    items.push(action(
      "status-approvals-review",
      &ctx.t("shell.status.approvals.review"),
      move || dispatch(BarAction::Go(route.clone())),
    ));
  }
  items
}

/// *Builder asks to run cargo add serde · KON-42*, or to use an MCP tool.
fn asks(ctx: &Ctx, shown: &ShownApproval) -> String {
  let approval = &shown.approval;
  let task = shown.task.as_deref().unwrap_or_default().to_owned();
  if approval.command {
    let command = match shown.first_line() {
      (line, true) => ctx
        .t_args("shell.status.approvals.firstLineMore", [("line", line)])
        .to_string(),
      (line, false) => line.to_owned(),
    };
    return ctx
      .t_args(
        "shell.status.approvals.asksCommand",
        [("agent", approval.agent.clone()), ("command", command), ("task", task)],
      )
      .to_string();
  }
  ctx
    .t_args(
      "shell.status.approvals.asksTool",
      [
        ("agent", approval.agent.clone()),
        ("server", approval.server.clone()),
        ("tool", approval.action.clone()),
        ("task", task),
      ],
    )
    .to_string()
}

/// *Auto-approving* and which run, or how many, with *Stop*; nothing when no run auto-approves.
pub fn auto_approving(ctx: &Ctx, runs: &[AutoApprovingRun], dispatch: &Dispatch) -> Vec<Element> {
  let Some(first) = runs.first() else {
    return Vec::new();
  };
  let which = match runs {
    [_] => ctx.t_args(
      "shell.status.approvals.autoOne",
      [("run", first.number.get().to_string()), ("task", first.key.to_string())],
    ),
    _ => ctx.t_args("shell.status.approvals.autoOther", [("count", runs.len().to_string())]),
  };
  let start = item("status-auto-approving")
    .child(glyph(Icon::CheckCheck, PaletteColor::TextPrimary))
    .child(label(&ctx.t("shell.status.approvals.auto"), PaletteColor::TextPrimary));
  let run = item("status-auto-approving-run")
    .flex_shrink(1.0)
    .shrink_order(give_way::RUN)
    .shrink_limit(ShrinkLimit::Drop)
    .child(label(&which, PaletteColor::TextSecondary));
  let ids: Vec<_> = runs.iter().map(|run| run.run).collect();
  let dispatch = dispatch.clone();
  vec![
    start.into(),
    run.into(),
    action(
      "status-auto-approving-stop",
      &ctx.t("shell.status.approvals.stop"),
      move || {
        dispatch(BarAction::StopAutoApproving(ids.clone()));
      },
    ),
  ]
}
