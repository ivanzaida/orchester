//! What a History row says under its title (`r9TeH` in `eFI39`): when the conversation last changed and how many
//! messages it holds, or when it was archived; of one that leads something, what it leads and when it last changed
//! after an 11 `eye` in `text_muted`, 5 before the words (`Meta` `B3sCcR` in `i687ei` c: *Leads 2 modules, 1 task ·
//! 12:20*).

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::ChatId, time::Timestamp};
use orchester_store::ChatListing;
use orchester_workspace::{display, orchestrator::watching::LeadCounts};

use crate::{
  theme::{ExtraSpacing, IconSize, OrchestratorText},
  ui::{
    icon::{Icon, icon},
    message::message_text,
  },
};

/// The line under the title of conversation `id`: `words` in their color, after the eye while it is `leading`.
pub(super) fn line(id: ChatId, (words, color): (&str, PaletteColor), leading: bool) -> Element {
  let text = Text::new(words)
    .id(format!("orchestrator-history-meta-{id}"))
    .variant(OrchestratorText::HistoryMeta.role())
    .color(color)
    .nowrap()
    .text_overflow(TextOverflow::Elipsis);
  if !leading {
    return text.width(Dimension::Pct(100.0)).into();
  }
  Row::new()
    .id(format!("orchestrator-history-leads-{id}"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Eye, IconSize::Xs, PaletteColor::TextMuted))
    .child(text.flex(1.0).min_width(Dimension::Px(0.0)))
    .into()
}

/// When `listing` last changed: its time `today`, else its date.
pub(super) fn when(ctx: &Ctx, listing: &ChatListing, today: bool) -> Arc<str> {
  if today {
    display::clock(listing.last).into()
  } else {
    super::super::date(ctx, listing.last)
  }
}

/// *Leads 2 modules, 1 task · 12:20*: what `listing` leads and when it last changed ([`when`]); `None` when it leads
/// nothing or is archived, whose row says when it was archived instead.
pub(super) fn leads(ctx: &Ctx, listing: &ChatListing, leads: &LeadCounts, today: bool) -> Option<Arc<str>> {
  if leads.is_empty() || listing.archived.is_some() {
    return None;
  }
  let parts: Vec<Arc<str>> = leads.parts().iter().map(|part| message_text(ctx, part)).collect();
  let separator = ctx.t("orchestrator.historyPanel.leadsSeparator");
  let what = parts.iter().map(|part| &**part).collect::<Vec<_>>().join(&separator);
  let when = when(ctx, listing, today);
  Some(ctx.t_args(
    "orchestrator.historyPanel.leads",
    [("what", what.as_str()), ("when", &*when)],
  ))
}

/// When `listing` last changed ([`when`]) and how many messages it holds; of an archived one, when it was archived
/// (*Archived today*, *Archived Oct 27*) instead.
pub(super) fn meta(ctx: &Ctx, listing: &ChatListing, today: bool) -> Arc<str> {
  let count = listing.messages.to_string();
  let messages = if listing.messages == 1 {
    ctx.t("orchestrator.historyPanel.messages.one")
  } else {
    ctx.t_args("orchestrator.historyPanel.messages.other", [("count", count.as_str())])
  };
  let Some(archived) = listing.archived else {
    let when = when(ctx, listing, today);
    return ctx.t_args(
      "orchestrator.historyPanel.meta",
      [("when", &*when), ("messages", &*messages)],
    );
  };
  if archived.local_day_start() == Timestamp::now().local_day_start() {
    return ctx.t_args("orchestrator.historyPanel.archivedToday", [("messages", &*messages)]);
  }
  let date = super::super::date(ctx, archived);
  ctx.t_args(
    "orchestrator.historyPanel.archivedOn",
    [("date", &*date), ("messages", &*messages)],
  )
}
