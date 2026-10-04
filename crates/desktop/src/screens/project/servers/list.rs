//! The servers at the left of the page (`E72eo` in 10.01): a 380 wide column under its overline,
//! each server a card with its name and connection label, its address in mono and who is signed in.
//! The shown server's card is on the hover fill; the others have a hairline and open on a press.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text, TextOverflow},
  layout::{Alignment, layout_kind::ScrollState},
  node::{Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::id::ServerId;

use super::labels::{account_line, connection_label, connection_name};
use crate::{
  navigation::route::{Page, ServerTab},
  screens::project::ProjectNav,
  sharing::ServerEntry,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    pressable::surface,
    status_label::status_label,
  },
};

/// The list column's width (`E72eo`).
const LIST_WIDTH: f32 = 380.0;

pub(super) fn server_list(
  ctx: &Ctx,
  entries: &[ServerEntry],
  shown: Option<ServerId>,
  nav: &ProjectNav,
  scroll: &ScrollState,
) -> Element {
  let cards = entries
    .iter()
    .map(|entry| card(ctx, entry, shown == Some(entry.stored.id), nav));
  let column = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Lg)
    .padding_bottom(SpacingSize::Lg)
    .padding_left(SpacingSize::Section)
    .padding_right(ExtraSpacing::Step.role())
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t("servers.list.overline")).variant(ExtraTypography::Overline.role()))
    .with_children(cards);
  Column::new()
    .id("server-list")
    .width(Dimension::Px(LIST_WIDTH))
    .height(Dimension::Pct(100.0))
    .border_right(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(
      ScrollVertical::new(column)
        .with_scroll_state(scroll.clone())
        .width(Dimension::Pct(100.0))
        .flex(1.0),
    )
    .into()
}

fn card(ctx: &Ctx, entry: &ServerEntry, selected: bool, nav: &ProjectNav) -> Element {
  let (glyph, state) = connection_label(ctx, entry);
  let name = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Server, IconSize::Dismiss, PaletteColor::TextPrimary))
    .child(
      Text::new(&entry.stored.name)
        .variant(TypographyStyle::Label)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0),
    )
    .child(Spacer::new().flex(1.0))
    .child(status_label(glyph, &state));
  let card = surface()
    .id(Arc::<str>::from(format!("server-{}", entry.stored.id)))
    .describe("connection", connection_name(entry.connection))
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Stack.role())
    .rounded(ExtraRadius::Notice.role())
    .child(lines(name, entry, &account_line(ctx, entry)));
  if selected {
    return card
      .background(PaletteColor::AccentMuted)
      .describe("state", "selected")
      .into();
  }
  nav
    .link(Page::Servers(Some(entry.stored.id), ServerTab::Account))
    .on(card.border(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role())))
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .into()
}

/// The name line, the address and the account line, 8 apart.
fn lines(name: Row, entry: &ServerEntry, account: &str) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(name)
    .child(
      Text::new(entry.address.as_str())
        .variant(TypographyStyle::Mono)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(
      Text::new(account)
        .variant(TypographyStyle::Caption)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
}
