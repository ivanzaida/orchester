//! Approvals in the status bar's activity slot ("Approval choices and waiting requests", `X3hVC`,
//! `rtv8N`): while the dialog is open, the request it shows and how many wait; after *Decide later*, how
//! many wait, the oldest of them, and *Review*, which opens the dialog on it. While a run auto-approves,
//! *Auto-approving* names it, with *Stop* (user decision of 2026-10-02; no frame yet).

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  node::Element,
};

use super::item::{
  GIVES_WAY_FIRST, GIVES_WAY_NEXT, GIVES_WAY_THEN, action, glyph, item_end, item_middle, item_start, label, separator,
  truncated_label, yielding_value,
};
use crate::{
  display,
  screens::project::{approval_queue::ApprovalQueue, command_line},
  tasks::{approvals::ShownApproval, data::AutoApprovingRun},
  theme::ExtraSpacing,
  ui::{icon::Icon, link::Press},
};

/// The waiting requests, oldest first; nothing when none waits. `dialog`: those of `pending` the
/// dialog may show on this page, the one it shows while it is open.
pub fn approvals(
  ctx: &Ctx,
  (pending, dialog): (&[ShownApproval], &[ShownApproval]),
  queue: Option<&ApprovalQueue>,
) -> Vec<Element> {
  let Some(oldest) = pending.first() else {
    return Vec::new();
  };
  let open = queue.and_then(|queue| queue.place(dialog, pending));
  let shown = open
    .as_ref()
    .and_then(|place| dialog.get(place.index))
    .unwrap_or(oldest);
  let asks = asks(ctx, shown);
  let count = pending.len().to_string();
  // What stays: the hand and, while the dialog is closed, how many wait; what the oldest asks gives way
  // first, and the time last.
  let (counted, detail) = match (&open, pending.len()) {
    (Some(_), 1) => (None, asks),
    (Some(_), _) => (
      None,
      ctx.t_args(
        "statusBar.approvals.openSeveral",
        [("asks", &*asks), ("count", count.as_str())],
      ),
    ),
    (None, 1) => (
      Some(ctx.t("statusBar.approvals.waitingCount.one")),
      ctx.t_args("statusBar.approvals.waitingAsks", [("asks", &*asks)]),
    ),
    (None, _) => (
      Some(ctx.t_args("statusBar.approvals.waitingCount.other", [("count", count.as_str())])),
      ctx.t_args("statusBar.approvals.waitingAsks", [("asks", &*asks)]),
    ),
  };
  let start = item_start("status-approvals")
    .describe("state", if open.is_some() { "open" } else { "waiting" })
    .flex_shrink(1.0)
    .background(PaletteColor::SurfaceRaised)
    .child(glyph(Icon::Hand, PaletteColor::TextPrimary));
  let start = match counted {
    Some(counted) => start.child(truncated_label(&counted, PaletteColor::TextPrimary)),
    // The words of the request follow the hand at the item's gap.
    None => start.padding_right(ExtraSpacing::Tight.role()),
  };
  let middle = item_middle("status-approvals-asks")
    .flex_shrink(GIVES_WAY_FIRST)
    .background(PaletteColor::SurfaceRaised)
    .child(truncated_label(&detail, PaletteColor::TextPrimary));
  let time = item_end("status-approvals-time")
    .padding_left(ExtraSpacing::Tight.role())
    .flex_shrink(GIVES_WAY_THEN)
    .background(PaletteColor::SurfaceRaised)
    .child(yielding_value(
      &display::clock(shown.approval.requested_at),
      PaletteColor::TextMuted,
    ));
  let mut items = vec![separator(), start.into(), middle.into(), time.into()];
  if let (None, Some(queue)) = (&open, queue) {
    let (queue, id) = (queue.clone(), oldest.approval.id.clone());
    items.push(action(
      "status-approvals-review",
      &ctx.t("statusBar.approvals.review"),
      Press::action(move || queue.review(&id)),
    ));
  }
  items
}

/// *Designer asks to use pencil batch_design · ORC-146*, or a command.
fn asks(ctx: &Ctx, shown: &ShownApproval) -> Arc<str> {
  let approval = &shown.approval;
  let task = shown.task.as_deref().unwrap_or_default();
  if approval.command {
    let command = command_line(ctx, shown);
    return ctx.t_args(
      "statusBar.approvals.asksCommand",
      [
        ("agent", approval.agent.as_str()),
        ("command", &*command),
        ("task", task),
      ],
    );
  }
  ctx.t_args(
    "statusBar.approvals.asksTool",
    [
      ("agent", approval.agent.as_str()),
      ("server", approval.server.as_str()),
      ("tool", approval.action.as_str()),
      ("task", task),
    ],
  )
}

/// *Auto-approving · Run 142 · ORC-142* (or how many runs) and *Stop*, which stops each of them.
pub fn auto_approving(ctx: &Ctx, runs: &[AutoApprovingRun], stop: impl Fn() + Send + Sync + 'static) -> Vec<Element> {
  let Some(first) = runs.first() else {
    return Vec::new();
  };
  let detail = match runs {
    [_] => ctx.t_args(
      "statusBar.approvals.autoOne",
      [("run", first.number.get().to_string().as_str()), ("task", &*first.key)],
    ),
    _ => ctx.t_args(
      "statusBar.approvals.autoOther",
      [("count", runs.len().to_string().as_str())],
    ),
  };
  // A safety indicator: *Auto-approving* and Stop keep their words. Which run it is gives way when the
  // bar is narrow, after what the waiting approvals ask and before the activity's words.
  let start = item_start("status-auto-approving")
    .flex_shrink(0.0)
    .child(glyph(Icon::Zap, PaletteColor::TextPrimary))
    .child(label(&ctx.t("statusBar.approvals.auto"), PaletteColor::TextPrimary));
  let run = item_end("status-auto-approving-run")
    .flex_shrink(GIVES_WAY_NEXT)
    .child(truncated_label(&detail, PaletteColor::TextPrimary));
  vec![
    separator(),
    start.into(),
    run.into(),
    action(
      "status-auto-approving-stop",
      &ctx.t("statusBar.approvals.stop"),
      Press::action(stop),
    ),
  ]
}
