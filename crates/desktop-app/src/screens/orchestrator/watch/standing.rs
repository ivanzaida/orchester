//! The line of an event of a target the conversation leads to the end
//! (`orchester_core::orchestrator::standing::StandingEvent`, `docs/plan/orchestrator-autonomy/OA-13-…`, `OA-15-…`):
//! *Work can start in Webhooks v2: KON-41, KON-42*, or *Webhooks v2 is stuck: 3 left for you*, and its time; the turn
//! it starts by itself answers under it. Drawn as the lines about what a conversation watches are (`avDZO`); its own
//! frame is the person's to draw (`docs/plan/orchestrator-autonomy/frames.md`).

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  node::Element,
};
use orchester_core::{
  orchestrator::standing::{Raised, StandingEvent},
  time::Timestamp,
};
use orchester_workspace::orchestrator::standing::event_line;

use super::{Said, line_row, time};
use crate::ui::{icon::Icon, message::message_text};

/// The line of `event`, kept at `at`; `id` names it.
pub(in crate::screens::orchestrator) fn line(
  ctx: &Ctx,
  id: &str,
  event: &StandingEvent,
  at: Option<Timestamp>,
) -> Element {
  let glyph = match event.raised {
    Raised::CanStart { .. } => (Icon::CirclePlay, PaletteColor::TextMuted),
    Raised::Stuck { .. } => (Icon::CircleAlert, PaletteColor::Warning),
  };
  let said = Said {
    glyph,
    words: message_text(ctx, &event_line(event)),
    tag: None,
    aside: None,
  };
  line_row(id, "standing", said, time(id, at)).into()
}
