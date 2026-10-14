//! Prompt source BYHVZ and excluded source S9rsK.
use lurq::{
  app::theme::{BorderSize, PaletteColor, TypographyStyle},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::pressable,
};
use crate::theme::{ExtraColor, ExtraRadius, ExtraSpacing, IconSize, instructions};
/// BYHVZ's numbered gutter.
const NUMBER_WIDTH: f32 = 14.0;
pub struct PromptSourceRow {
  pub number: usize,
  pub kind: String,
  pub name: String,
  pub version: String,
  pub selected: bool,
}
impl PromptSourceRow {
  pub fn render(self, id: &str, open: impl Fn() + Send + Sync + 'static) -> Element {
    let row = pressable(open)
      .id(id.to_owned())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(
        ExtraSpacing::Loose.role(),
        ExtraSpacing::TextArea.role(),
      ))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .rounded(ExtraRadius::Tile.role())
      .hovered_style(Style::new().background(ExtraColor::Hover.role()));
    let row = if self.selected {
      row.background(ExtraColor::Hover.role())
    } else {
      row
    };
    row
      .child(
        Text::new(&self.number.to_string())
          .variant(TypographyStyle::Mono)
          .color(PaletteColor::TextMuted)
          .width(Dimension::Px(NUMBER_WIDTH)),
      )
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(ExtraSpacing::Hair.role())
          .child(Text::new(&self.kind).variant(TypographyStyle::Caption))
          .child(Text::new(&self.name).variant(instructions::rule_name())),
      )
      .child(
        Text::new(&self.version)
          .variant(instructions::variable_value())
          .color(PaletteColor::TextSecondary)
          .nowrap(),
      )
      .into()
  }
}
pub fn excluded_source(id: &str, title: &str, reason: &str) -> Element {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ExtraSpacing::Loose.role(),
      ExtraSpacing::TextArea.role(),
    ))
    .spacing(ExtraSpacing::Loose.role())
    .rounded(ExtraRadius::Tile.role())
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .child(icon(Icon::EyeOff, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(Text::new(title).variant(TypographyStyle::Description))
        .child(Text::new(reason).variant(TypographyStyle::Caption)),
    )
    .into()
}
