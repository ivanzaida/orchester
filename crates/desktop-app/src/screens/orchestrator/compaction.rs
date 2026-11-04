//! A compaction of the conversation's session as its event line (ORC-309, `docs/design/context-meter.md`): while it
//! runs a `loader-circle` in `text_secondary`, *Compacting the conversation* and the tokens it started from
//! (`dPq1I`); once done a `fold-vertical` in `text_muted`, *Compacted* and the tokens before and after (`l5YmWb`), or
//! that the CLI reports the size after it only with the next reply; one that failed says so in `danger`, why on hover
//! and under the line once pressed. Each says when it started.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  orchestrator::{ChatEvent, CompactionState},
  time::Timestamp,
  tokens::compact_tokens,
};
use orchester_workspace::display;

use super::line::{Line, Tone};
use crate::ui::icon::Icon;

/// The line of compaction `event`, started at `at`, named `id`; `None` for another event.
pub(super) fn line(ctx: &mut Ctx, id: &str, event: &ChatEvent, at: Option<Timestamp>) -> Option<lurq::node::Element> {
  let ChatEvent::Compaction { before, state } = event else {
    return None;
  };
  let before = before.map(compact_tokens);
  let (glyph, tone, what, status) = match state {
    CompactionState::Running => (
      Icon::LoaderCircle,
      Tone::Plain,
      "orchestrator.context.event.compacting",
      "running",
    ),
    CompactionState::Done { .. } => (
      Icon::FoldVertical,
      Tone::Quiet,
      "orchestrator.context.event.compacted",
      "done",
    ),
    CompactionState::Failed { .. } => (
      Icon::CircleAlert,
      Tone::Danger,
      "orchestrator.context.event.notCompacted",
      "failed",
    ),
  };
  let target: Option<Arc<str>> = match (state, before.as_deref()) {
    (CompactionState::Done { after: Some(after) }, Some(before)) => Some(ctx.t_args(
      "orchestrator.context.event.change",
      [("before", before), ("after", compact_tokens(*after).as_str())],
    )),
    (CompactionState::Done { after: Some(after) }, None) => Some(ctx.t_args(
      "orchestrator.context.event.after",
      [("after", compact_tokens(*after).as_str())],
    )),
    (CompactionState::Done { after: None }, Some(before)) => {
      Some(ctx.t_args("orchestrator.context.event.afterUnreported", [("before", before)]))
    }
    (_, before) => before.map(Arc::from),
  };
  let detail = match state {
    CompactionState::Failed { reason } => Some(super::failure::why(ctx, reason)),
    _ => None,
  };
  let line = Line::new(id, (glyph, tone), ctx.t(what))
    .outcome("compaction")
    .status(status)
    .target(target)
    .time(at.map(display::clock))
    .detail(detail);
  Some(line.render(ctx))
}
