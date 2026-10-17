//! The parts Sharing's questions and pages share: a question in place under its row (`Confirm · Make Kontur local on
//! this computer?` in `woioY`): its title in 13/600, what it does in 12/1.5 `text_secondary`, what happens to each
//! thing as pairs, and its buttons at the end, 10 apart, 14 over and under and 16 at the sides, on `surface_raised` in
//! a `border_strong` stroke, radius 8; and a panel of pairs (`Panel · What is uploaded` in `KSnOW`): its title, then
//! the pairs, 8 apart, 12 over and under and 14 at the sides, on `surface_raised` in a `border` stroke, radius 8.

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::theme::{ExtraRadius, ExtraSpacing, ExtraTypography, OrchesterText};

/// What a pair is, in a question (`Key` `pIuer` in `woioY`) and in a panel (`L3zZNK` in `KSnOW`).
pub(super) const QUESTION_KEY: f32 = 190.0;
pub(super) const PANEL_KEY: f32 = 150.0;

/// What happens to one thing: its name in 12/1.45 `text_muted` in a `key` column, and the value 12 after it in
/// `text_primary`, wrapping.
pub(super) fn pair(id: &str, (key, width): (&str, f32), value: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .child(
      Text::new(key)
        .id(format!("{id}-key"))
        .variant(ExtraTypography::Prose.role())
        .color(PaletteColor::TextMuted)
        .width(Dimension::Px(width))
        .flex_shrink(0.0),
    )
    .child(
      Text::new(value)
        .id(format!("{id}-value"))
        .variant(ExtraTypography::Prose.role())
        .color(PaletteColor::TextPrimary)
        .flex(1.0),
    )
}

/// A question in place with its `title`, its `body`, the `pairs` and its `actions`, in the given order.
pub(super) fn question(id: &str, (title, body): (&str, &str), pairs: Vec<Row>, actions: Vec<Element>) -> Column {
  let text = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(ExtraTypography::FormSection.role()),
    )
    .child(
      Text::new(body)
        .id(format!("{id}-body"))
        .variant(TypographyStyle::Description)
        .width(Dimension::Pct(100.0)),
    );
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
    .spacing(ExtraSpacing::Loose.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(ExtraRadius::Tile.role())
    .child(text)
    .with_children(pairs)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(Spacer::new().flex(1.0))
        .with_children(actions),
    )
}

/// A panel of pairs under its `title`.
pub(super) fn panel(id: &str, title: &str, pairs: Vec<Row>) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(OrchesterText::CardTitle.role()),
    )
    .with_children(pairs)
    .into()
}
