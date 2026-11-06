//! The line of a subscription that ended by itself (`avDZO` d): *Every task of Mobile release is Done* with a `flag`,
//! in `success` once all its work is Done, then *— watching ended; the module's summary is below* while the turn it
//! starts by itself writes that summary under it, and its time.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  node::Element,
};
use orchester_core::{
  orchestrator::{
    WatchTarget,
    watch::{EndReason, WatchSummary},
  },
  time::Timestamp,
};

use super::{Said, line_row, time};
use crate::ui::icon::Icon;

/// The line of `summary`, a subscription that ended by itself, kept at `at`; `id` names it.
pub(in crate::screens::orchestrator) fn line(
  ctx: &Ctx,
  id: &str,
  summary: &WatchSummary,
  at: Option<Timestamp>,
) -> Element {
  let name = summary.name.as_str();
  let (key, done) = match summary.reason {
    EndReason::TaskDone => (("orchestrator.watch.ended.taskDone", "task"), true),
    EndReason::TaskCanceled => (("orchestrator.watch.ended.taskCanceled", "task"), false),
    EndReason::ModuleClosed { canceled: 0, .. } => (("orchestrator.watch.ended.moduleDone", "module"), true),
    EndReason::ModuleClosed { done: 0, .. } => (("orchestrator.watch.ended.moduleCanceled", "module"), false),
    EndReason::ModuleClosed { .. } => (("orchestrator.watch.ended.moduleClosed", "module"), false),
  };
  let aside = match (summary.automatic, summary.target) {
    (true, WatchTarget::Task(_)) => "orchestrator.watch.ended.asideTask",
    (true, _) => "orchestrator.watch.ended.asideModule",
    (false, _) => "orchestrator.watch.ended.aside",
  };
  let color = if done {
    PaletteColor::Success
  } else {
    PaletteColor::TextMuted
  };
  let (words, placeholder) = key;
  let said = Said {
    glyph: (Icon::Flag, color),
    words: ctx.t_args(words, [(placeholder, name)]),
    aside: Some(ctx.t(aside)),
  };
  line_row(id, "ended", said, time(id, at)).into()
}
