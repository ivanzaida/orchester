//! The line of a subscription that ended by itself (`avDZO` d): *Every task of Mobile release is Done* with a `flag`,
//! in `success` once all its work is Done, then *— watching ended; the module's summary is below* while the turn it
//! starts by itself writes that summary under it, and its time. The words and when they mark a success are
//! `orchester_workspace::orchestrator::watch_ends`'.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  node::Element,
};
use orchester_core::{orchestrator::watch::WatchSummary, time::Timestamp};
use orchester_workspace::orchestrator::watch_ends::{ended_aside, ended_done, ended_line};

use super::{Said, line_row, time};
use crate::ui::{icon::Icon, message::message_text};

/// The line of `summary`, a subscription that ended by itself, kept at `at`; `id` names it.
pub(in crate::screens::orchestrator) fn line(
  ctx: &Ctx,
  id: &str,
  summary: &WatchSummary,
  at: Option<Timestamp>,
) -> Element {
  let color = if ended_done(summary.reason) {
    PaletteColor::Success
  } else {
    PaletteColor::TextMuted
  };
  let said = Said {
    glyph: (Icon::Flag, color),
    words: message_text(ctx, &ended_line(summary)),
    tag: None,
    aside: Some(message_text(ctx, &ended_aside(summary))),
  };
  line_row(id, "ended", said, time(id, at)).into()
}
