//! The details panel's parts (`Account details` `S9wde` in `Cu20b`, `Server details` `v9NL4N` in `SFaAp`): the card,
//! its header with a 32 glyph tile, the title with the state after it and the facts under them, the detail tabs, and
//! a section with its title (13/600, what it counts at its end) and its words (12.5, 1.5).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{list::state_line, words::Look};
use crate::{
  theme::{AccountsSpacing, AccountsText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    tabs::{PageTab, page_tabs},
  },
};

/// The header's glyph tile (`E8zDk`).
const GLYPH_TILE: f32 = 32.0;

/// The right panel's card.
pub(super) fn frame(id: &str) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
}

/// A fact in a header's line: words, or an address or an email in mono (`TKeDb` in `Cu20b`).
pub(super) enum Fact {
  Plain(String),
  Mono(String),
}

/// The header (`S4iAdc`): the glyph, the title with its state, what follows the state (the account's two-factor
/// authentication, `WH6nX`) and the facts under them, and the actions at the end.
pub(super) fn header(
  ctx: &Ctx,
  id: &str,
  (glyph, title, look): (Icon, &str, &Look),
  (after, facts): (Vec<Element>, &[Fact]),
) -> Row {
  let separator = ctx.t("cloud.factSeparator");
  let mut line = Row::new()
    .id(format!("{id}-facts"))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center);
  for (index, fact) in facts.iter().enumerate() {
    if index > 0 {
      line = line.child(Text::new(&separator).variant(ExtraTypography::Note.role()));
    }
    line = line.child(match fact {
      Fact::Plain(text) => Text::new(text).variant(ExtraTypography::Note.role()).nowrap(),
      Fact::Mono(text) => Text::new(text).variant(ExtraTypography::MonoKey.role()).nowrap(),
    });
  }
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Xs)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Text::new(title)
            .id(format!("{id}-title"))
            .variant(AccountsText::Title.role())
            .nowrap(),
        )
        .child(state_line(ctx, &format!("{id}-state"), look))
        .with_children(after),
    )
    .child(line);
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(AccountsSpacing::Panel.role(), SpacingSize::Lg))
    .spacing(AccountsSpacing::Cell.role())
    .align_items(Alignment::Start)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(tile(glyph))
    .child(text)
}

fn tile(glyph: Icon) -> Row {
  Row::new()
    .width(Dimension::Px(GLYPH_TILE))
    .height(Dimension::Px(GLYPH_TILE))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Sm)
    .child(icon(glyph, IconSize::Nav, PaletteColor::TextSecondary))
}

/// The detail tabs (`sjwEp`).
pub(super) fn tabs(ctx: &mut Ctx, id: &str, tabs: impl IntoIterator<Item = PageTab>) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(AccountsSpacing::Panel.role()))
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(page_tabs(ctx, id, tabs))
    .into()
}

/// The panel's body (`v8eu7`): its parts 20 apart inside 18, scrolling.
pub(super) fn body(id: &str, parts: Vec<Element>) -> Element {
  ScrollVertical::new(
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::all(AccountsSpacing::Panel.role()))
      .spacing(AccountsSpacing::Parts.role())
      .with_children(parts),
  )
  .id(id.to_owned())
  .flex(1.0)
  .into()
}

/// A section's title (`XVbf0`): 13/600, what it counts at its end.
pub(super) fn section_title(id: &str, title: &str, meta: Option<&str>) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(title).variant(ExtraTypography::FormSection.role()))
    .child(Spacer::new().flex(1.0))
    .with_children(meta.map(|meta| {
      Text::new(meta)
        .id(format!("{id}-meta"))
        .variant(TypographyStyle::Caption)
    }))
}

/// What a section says (`ysTEY`): 12.5 `text_secondary`, wrapping.
pub(super) fn paragraph(id: &str, text: &str) -> Text {
  Text::new(text)
    .id(id.to_owned())
    .variant(ExtraTypography::Lead.role())
    .width(Dimension::Pct(100.0))
}

/// A hint under a section's actions (`FVLQK`): 11.5 `text_muted`, wrapping.
pub(super) fn hint(id: &str, text: &str) -> Text {
  Text::new(text)
    .id(id.to_owned())
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0))
}

/// A section: its title, then its parts 8 apart.
pub(super) fn section(id: &str, title: Row, parts: Vec<Element>) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(title)
    .with_children(parts)
}
