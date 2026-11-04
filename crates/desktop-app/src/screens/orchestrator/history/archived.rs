//! History's archived conversations (`08d · History · Archived` `qAfeu`): at the panel's top a head 8 over and under
//! and 12 at its sides (`Archived head` `L48hkP`), its title row (a back arrow in a 24 square, *Archived* in 12.5/500
//! and their count in Geist Mono 11 `text_muted`, 6 apart) over *Search archived*, 8 under it; the list, 4 under the
//! head, the one archived last first, each row ([`super::row`]) saying when it was archived and how many messages it
//! holds and offering *Unarchive* under the pointer; and at its foot a note that they keep every message. The arrow
//! goes back to History's list.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ChatId, time::Timestamp};
use orchester_store::ChatListing;

use super::{HistoryState, Item, Rows, chat_item, found, list, search_words};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, OrchestratorText},
  ui::{field::FieldInput, icon::Icon, icon_button::IconButton},
};

/// The head, the list and the foot of the view over the `archived` conversations of the project `rows` name, `current`
/// the one shown, searched by `state`'s own words.
pub(super) fn parts(
  ctx: &mut Ctx,
  state: &HistoryState,
  rows: &Rows,
  (archived, current): (&[&ChatListing], Option<ChatId>),
) -> (Element, Element, Element) {
  let back = state.archived.clone();
  let title = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      IconButton::new(
        "orchestrator-history-archived-back",
        Icon::ArrowLeft,
        ctx.t("orchestrator.historyPanel.backToHistory"),
      )
      .small()
      .on_click(move || back.set(false))
      .render(ctx),
    )
    .child(
      Text::new(&ctx.t("orchestrator.historyPanel.archived"))
        .id("orchestrator-history-archived-title")
        .variant(OrchestratorText::Emphasis.role())
        .nowrap(),
    )
    .child(Text::new(&archived.len().to_string()).variant(ExtraTypography::MonoNote.role()));
  let label = ctx.t("orchestrator.historyPanel.searchArchived");
  let field = FieldInput::new(
    "orchestrator-history-archived-search",
    label.clone(),
    state.archived_search.clone(),
  )
  .search()
  .placeholder(label)
  .render(ctx);
  let head = Column::new()
    .id("orchestrator-history-archived-head")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .child(title)
    .child(field);
  let words = search_words(&state.archived_search.get());
  let today = Timestamp::now().local_day_start();
  let items: Vec<Item> = archived
    .iter()
    .filter(|listing| found(listing, &words))
    .map(|listing| chat_item(listing, (current, rows), today))
    .collect();
  let empty = if archived.is_empty() {
    "orchestrator.historyPanel.archivedEmpty"
  } else {
    "orchestrator.historyPanel.archivedNoMatch"
  };
  let list = Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .padding(Padding::new().top(SpacingSize::Xs))
    .child(list(ctx, items, ("orchestrator-history-archived-list", empty)));
  let foot = Column::new()
    .id("orchestrator-history-foot")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Loose.role()))
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&ctx.t("orchestrator.historyPanel.archivedNote"))
        .id("orchestrator-history-note")
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    );
  (head.into(), list.into(), foot.into())
}
