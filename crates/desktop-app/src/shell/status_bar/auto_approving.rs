//! Runs that answer their own requests (handover §3.3, `11 · Runs auto-approving` `X9S93` in `Status bar ·
//! redesign`): *Auto-approving · Run 142 · KON-42* (or how many runs) in `warning`, with *Stop*, which stops each of
//! them. When the bar is narrow the run drops out; *Stop* stays. Waiting requests show in the title bar's *Needs
//! you*, not here.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  layout::layout_kind::ShrinkLimit,
  node::Element,
};
use orchester_workspace::tasks::data::AutoApprovingRun;

use super::{
  facts::{BarAction, Dispatch, Stopping},
  item::{action, cut, give_way, glyph, item, label},
};
use crate::ui::{icon::Icon, message::message_text, pressable::Press};

/// *Auto-approving* and which run, or how many, with *Stop*; nothing when no run auto-approves. While the
/// runs are told to stop, *Stop* waits; when they could not be, why follows it, and *Stop* can be
/// pressed again.
pub fn auto_approving(
  ctx: &mut Ctx,
  runs: &[AutoApprovingRun],
  stopping: &Stopping,
  dispatch: &Dispatch,
) -> Vec<Element> {
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
    .child(glyph(Icon::CheckCheck, PaletteColor::Warning))
    .child(label(&ctx.t("shell.status.approvals.auto"), PaletteColor::Warning));
  let run = item("status-auto-approving-run")
    .flex_shrink(1.0)
    .shrink_order(give_way::RUN)
    .shrink_limit(ShrinkLimit::Drop)
    .child(label(&which, PaletteColor::Warning));
  let mut items = vec![start.into(), run.into()];
  if *stopping == Stopping::Busy {
    items.push(
      item("status-auto-approving-stop")
        .describe("state", "busy")
        .child(glyph(Icon::LoaderCircle, PaletteColor::TextMuted))
        .child(label(
          &ctx.t("shell.status.approvals.stopping"),
          PaletteColor::TextMuted,
        ))
        .into(),
    );
    return items;
  }
  let ids: Vec<_> = runs.iter().map(|run| run.run).collect();
  let dispatch = dispatch.clone();
  items.push(action(
    "status-auto-approving-stop",
    &ctx.t("shell.status.approvals.stop"),
    Press::run(move || dispatch(BarAction::StopAutoApproving(ids.clone()))),
  ));
  if let Stopping::Failed(reason) = stopping {
    let text = ctx.t_args(
      "shell.status.approvals.stopFailed",
      [("reason", message_text(ctx, reason).to_string())],
    );
    items.push(
      item("status-auto-approving-error")
        .flex_shrink(1.0)
        .shrink_order(give_way::ACTIVITY)
        .shrink_limit(ShrinkLimit::Content)
        .child(glyph(Icon::CircleAlert, PaletteColor::TextPrimary))
        .child(cut(
          ctx,
          "status-auto-approving-error-text",
          &text,
          PaletteColor::TextPrimary,
        ))
        .into(),
    );
  }
  items
}
