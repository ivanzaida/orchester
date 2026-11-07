//! What started an Orchestrator turn by itself, as a tag on its answer (`Started by` `S1Agi` in `04 · Leading two
//! modules` `OKtX2` of `Orchestrator watch · redesign`): 18 high on `surface_raised`, radius 4, 6 at its sides, an 11
//! `zap` and its words 4 apart, both 11 `text_muted`: *KON-41's wait*, *KON-52 merged*. An answer with a name line has
//! it after the time; one nested under a wait's line has it first in the quote.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::orchestrator::{AgentWait, TaskEventKind, WaitKind};
use orchester_workspace::orchestrator::chat::EventFacts;

use crate::{
  theme::{ExtraSpacing, IconSize},
  ui::icon::{Icon, icon},
};

/// The tag's height (`S1Agi`).
const HEIGHT: f32 = 18.0;

/// The tag of the turn `wait` started by itself, with what its event names beside it (`facts`); `id` names it.
pub(super) fn tag(ctx: &Ctx, id: &str, wait: &AgentWait, facts: Option<&EventFacts>) -> Element {
  Row::new()
    .id(id.to_owned())
    .flex_shrink(0.0)
    .height(Dimension::Px(HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Sm)
    .child(icon(Icon::Zap, IconSize::Xs, PaletteColor::TextMuted))
    .child(
      Text::new(&words(ctx, wait, facts))
        .id(format!("{id}-label"))
        .variant(TypographyStyle::Caption)
        .nowrap(),
    )
    .into()
}

/// What the tag says started the turn: the wait or the run's end of its task, or what happened to it; for a task the
/// run waits on, that task, once it is known.
fn words(ctx: &Ctx, wait: &AgentWait, facts: Option<&EventFacts>) -> Arc<str> {
  let waited = facts
    .and_then(|facts| facts.waited.as_ref())
    .map(|waited| waited.key.as_str());
  let (key, task) = match wait.kind {
    WaitKind::Question | WaitKind::Message | WaitKind::Plan => ("orchestrator.startedBy.wait", None),
    WaitKind::Ended(_) => ("orchestrator.startedBy.runEnd", None),
    WaitKind::Event(event) => match event.kind {
      TaskEventKind::NoProgress { .. } => ("orchestrator.startedBy.noProgress", None),
      TaskEventKind::MergeConflict => ("orchestrator.startedBy.conflict", None),
      TaskEventKind::WaitedMerged { .. } if waited.is_some() => ("orchestrator.startedBy.merged", waited),
      TaskEventKind::WaitedClosed { .. } if waited.is_some() => ("orchestrator.startedBy.closed", waited),
      TaskEventKind::WaitedMerged { .. } | TaskEventKind::WaitedClosed { .. } => {
        ("orchestrator.startedBy.waited", None)
      }
    },
  };
  ctx.t_args(key, [("task", task.unwrap_or(wait.task.as_str()))])
}
