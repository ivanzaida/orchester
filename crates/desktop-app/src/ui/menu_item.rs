//! A menu row (`Menu item` `mAHGP`) and the line between groups (`Menu separator` `UoDdI`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  nav_item::NAV_ITEM_HEIGHT,
  pressable::pressable,
};
use crate::theme::{ExtraColor, IconSize};

pub struct MenuItem<'a> {
  pub id: &'static str,
  pub icon: Icon,
  pub label: &'a str,
}

impl MenuItem<'_> {
  pub fn on_click(self, handler: impl Fn() + Send + Sync + 'static) -> Element {
    let row = Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(NAV_ITEM_HEIGHT))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(self.icon, IconSize::Md, PaletteColor::TextSecondary))
      .child(
        Text::new(self.label)
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextPrimary)
          .nowrap()
          .flex(1.0),
      );
    pressable(handler)
      .id(self.id)
      .describe("a11y_name", self.label)
      .width(Dimension::Pct(100.0))
      .rounded(RadiusSize::Sm)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .child(row)
      .into()
  }
}

/// The line between groups, `border_size_sm` thick.
pub fn separator(ctx: &mut Ctx) -> Element {
  let line = ctx.theme().border_size_value(BorderSize::Sm);
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Xs))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(line))
        .background(PaletteColor::Border),
    )
    .into()
}
