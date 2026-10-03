//! Approvals in the status bar's activity slot ("Approval choices and waiting requests", `X3hVC`,
//! `rtv8N`): while the dialog is open, the request it shows and how many wait; after *Decide later*, how
//! many wait, the oldest of them, and *Review*, which opens the dialog on it. While a run auto-approves,
//! *Auto-approving* names it, with *Stop* (user decision of 2026-10-02; no frame yet).

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
  layout::{Alignment, layout_kind::ShrinkLimit},
  node::{Element, dimension::Dimension},
};

use super::item::{
  action, dropping_label, dropping_value, give_way, glyph, item_end, item_middle, item_start, label, separator,
  truncated_label,
};
use crate::{
  display,
  screens::project::{approval_queue::ApprovalQueue, command_line},
  tasks::{approvals::ShownApproval, data::AutoApprovingRun},
  theme::{APPROVALS_ASKS_MIN, ExtraSpacing},
  ui::{icon::Icon, link::Press},
};

/// The waiting requests, oldest first; nothing when none waits. `dialog`: those of `pending` the
/// dialog may show on this page, the one it shows while it is open. When the bar is narrow, the parts
/// give way in their order ([`give_way`]): what the oldest asks trims its end down to
/// [`APPROVALS_ASKS_MIN`], the time drops out, then the words beside the count; the hand, the count
/// and *Review* stay.
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
  let count = pending.len();
  let start = item_start("status-approvals")
    .describe("state", if open.is_some() { "open" } else { "waiting" })
    .flex_shrink(1.0)
    .shrink_order(give_way::COUNT_WORDS)
    .shrink_limit(ShrinkLimit::Content)
    .background(PaletteColor::SurfaceRaised)
    .child(glyph(Icon::Hand, PaletteColor::TextPrimary));
  // While the dialog is open, what it shows is the item's text; otherwise how many wait comes first.
  let (start, detail) = match (&open, count) {
    (Some(_), 1) => (start.padding_right(ExtraSpacing::Tight.role()), asks),
    (Some(_), _) => (
      start.padding_right(ExtraSpacing::Tight.role()),
      ctx.t_args(
        "statusBar.approvals.openSeveral",
        [("asks", &*asks), ("count", count.to_string().as_str())],
      ),
    ),
    (None, _) => (
      start.child(count_line(ctx, count)),
      ctx.t_args("statusBar.approvals.waitingAsks", [("asks", &*asks)]),
    ),
  };
  let middle = item_middle("status-approvals-asks")
    .flex_shrink(1.0)
    .shrink_order(give_way::ASKS)
    .min_width(Dimension::Px(APPROVALS_ASKS_MIN))
    .background(PaletteColor::SurfaceRaised)
    .child(truncated_label(&detail, PaletteColor::TextPrimary));
  // The end closes the item; its time drops out whole.
  let end = item_end("status-approvals-time")
    .flex_shrink(1.0)
    .shrink_order(give_way::TIMES)
    .shrink_limit(ShrinkLimit::Content)
    .padding_left(ExtraSpacing::Tight.role())
    .background(PaletteColor::SurfaceRaised)
    .child(dropping_value(
      &display::clock(shown.approval.requested_at),
      PaletteColor::TextMuted,
    ));
  let mut items = vec![separator(), start.into(), middle.into(), end.into()];
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

/// *2 approvals waiting*: the count, and the words after it, which drop out when the bar is narrow.
fn count_line(ctx: &Ctx, count: usize) -> Row {
  let words = ctx.t(if count == 1 {
    "statusBar.approvals.waitingWords.one"
  } else {
    "statusBar.approvals.waitingWords.other"
  });
  Row::new()
    .flex_shrink(1.0)
    .shrink_limit(ShrinkLimit::Content)
    .align_items(Alignment::Center)
    .child(label(&count.to_string(), PaletteColor::TextPrimary))
    .child(dropping_label(&words, PaletteColor::TextPrimary))
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
  // A safety indicator: *Auto-approving* and Stop keep their words; which run it is drops out whole when
  // the bar is narrow, after what the waiting approvals ask has trimmed its end ([`give_way`]).
  let start = item_start("status-auto-approving")
    .flex_shrink(0.0)
    .child(glyph(Icon::Zap, PaletteColor::TextPrimary))
    .child(label(&ctx.t("statusBar.approvals.auto"), PaletteColor::TextPrimary));
  let run = item_end("status-auto-approving-run")
    .flex_shrink(1.0)
    .shrink_order(give_way::RUN)
    .shrink_limit(ShrinkLimit::Drop)
    .child(label(&detail, PaletteColor::TextPrimary));
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
