//! What a History row says under its title (`r9TeH` in `eFI39`): when the conversation last changed and how many
//! messages it holds, or when it was archived.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::time::Timestamp;
use orchester_store::ChatListing;
use orchester_workspace::display;

/// When `listing` last changed: its time `today`, else its date.
pub(super) fn when(ctx: &Ctx, listing: &ChatListing, today: bool) -> Arc<str> {
  if today {
    display::clock(listing.last).into()
  } else {
    super::super::date(ctx, listing.last)
  }
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
