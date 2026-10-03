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
  GIVES_WAY_FIRST, GIVES_WAY_NEXT, action, glyph, item_end, item_middle, item_start, label, separator, truncated_label,
  value,
};
use crate::{
  display,
  screens::project::{approval_queue::ApprovalQueue, command_line},
  tasks::{approvals::ShownApproval, data::AutoApprovingRun},
  theme::{ApprovalsWidths, ExtraSpacing},
  ui::{icon::Icon, link::Press},
};

/// Which parts of the waiting approvals, and of the run that auto-approves beside them, the window is
/// wide enough for ([`ApprovalsWidths`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApprovalParts {
  pub long_count: bool,
  pub time: bool,
  pub run: bool,
  pub asks: bool,
}

impl ApprovalParts {
  pub fn at(width: f32, widths: ApprovalsWidths) -> Self {
    Self {
      long_count: width >= widths.long_count,
      time: width >= widths.time,
      run: width >= widths.run,
      asks: width >= widths.asks,
    }
  }
}

/// How the auto-approving item names its run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunPart {
  /// Whole, beside waiting approvals with room for it.
  Whole,
  /// Not at all, beside waiting approvals without room for it.
  Hidden,
  /// Without waiting approvals: it gives way with an ellipsis before the activity's words.
  GivesWay,
}

/// The waiting requests, oldest first; nothing when none waits. `dialog`: those of `pending` the
/// dialog may show on this page, the one it shows while it is open. `parts`: which parts the window
/// is wide enough for ([`ApprovalParts`]); each shows whole or not at all.
pub fn approvals(
  ctx: &Ctx,
  (pending, dialog): (&[ShownApproval], &[ShownApproval]),
  queue: Option<&ApprovalQueue>,
  parts: ApprovalParts,
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
  // The hand and how many wait stay; what the oldest asks follows when there is room for it. While the
  // dialog is open, what it shows is the item's text, and the count stands in for it when it has no room.
  let (counted, detail) = match (&open, count) {
    (Some(_), _) if !parts.asks => (Some(count_text(ctx, count, parts.long_count)), None),
    (Some(_), 1) => (None, Some(asks)),
    (Some(_), _) => (
      None,
      Some(ctx.t_args(
        "statusBar.approvals.openSeveral",
        [("asks", &*asks), ("count", count.to_string().as_str())],
      )),
    ),
    (None, _) => (
      Some(count_text(ctx, count, parts.long_count)),
      parts
        .asks
        .then(|| ctx.t_args("statusBar.approvals.waitingAsks", [("asks", &*asks)])),
    ),
  };
  let start = item_start("status-approvals")
    .describe("state", if open.is_some() { "open" } else { "waiting" })
    // Only where nothing else fits (a CLI's limit reached, at 960): how many wait is cut last.
    .flex_shrink(1.0)
    .background(PaletteColor::SurfaceRaised)
    .child(glyph(Icon::Hand, PaletteColor::TextPrimary));
  let start = match counted {
    Some(counted) => start.child(truncated_label(&counted, PaletteColor::TextPrimary)),
    // The words of the request follow the hand at the item's gap.
    None => start.padding_right(ExtraSpacing::Tight.role()),
  };
  let middle = detail.map(|detail| {
    item_middle("status-approvals-asks")
      .flex_shrink(GIVES_WAY_FIRST)
      .background(PaletteColor::SurfaceRaised)
      .child(truncated_label(&detail, PaletteColor::TextPrimary))
  });
  // The end closes the item, with the time when there is room for it.
  let time = parts
    .time
    .then(|| value(&display::clock(shown.approval.requested_at), PaletteColor::TextMuted));
  let end = item_end("status-approvals-time")
    .flex_shrink(0.0)
    .background(PaletteColor::SurfaceRaised);
  let end = match time {
    Some(time) => end.padding_left(ExtraSpacing::Tight.role()).child(time),
    None => end,
  };
  let mut items = vec![separator(), start.into()];
  items.extend(middle.map(Into::into));
  items.push(end.into());
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

/// *2 approvals waiting*, or *2 waiting* when the window has no room for the long form.
fn count_text(ctx: &Ctx, count: usize, long: bool) -> Arc<str> {
  match (long, count) {
    (true, 1) => ctx.t("statusBar.approvals.waitingCount.one"),
    (true, _) => ctx.t_args(
      "statusBar.approvals.waitingCount.other",
      [("count", count.to_string().as_str())],
    ),
    (false, 1) => ctx.t("statusBar.approvals.waitingBrief.one"),
    (false, _) => ctx.t_args(
      "statusBar.approvals.waitingBrief.other",
      [("count", count.to_string().as_str())],
    ),
  }
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
pub fn auto_approving(
  ctx: &Ctx,
  runs: &[AutoApprovingRun],
  part: RunPart,
  stop: impl Fn() + Send + Sync + 'static,
) -> Vec<Element> {
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
  // A safety indicator: *Auto-approving* and Stop keep their words. Which run it is shows as `part` says.
  let start = item_start("status-auto-approving")
    .flex_shrink(0.0)
    .child(glyph(Icon::Zap, PaletteColor::TextPrimary))
    .child(label(&ctx.t("statusBar.approvals.auto"), PaletteColor::TextPrimary));
  let run = match part {
    RunPart::Whole => Some(
      item_end("status-auto-approving-run")
        .flex_shrink(0.0)
        .child(label(&detail, PaletteColor::TextPrimary)),
    ),
    RunPart::GivesWay => Some(
      item_end("status-auto-approving-run")
        .flex_shrink(GIVES_WAY_NEXT)
        .child(truncated_label(&detail, PaletteColor::TextPrimary)),
    ),
    RunPart::Hidden => None,
  };
  let mut items = vec![separator(), start.into()];
  items.extend(run.map(Into::into));
  items.push(action(
    "status-auto-approving-stop",
    &ctx.t("statusBar.approvals.stop"),
    Press::action(stop),
  ));
  items
}
