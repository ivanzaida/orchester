//! A sidebar navigation item (`Nav item/active` `q8fWpF`, `Nav item/default` `RAsOv`): icon and label,
//! and at its end a count in 11 `text_muted` (`Meta`), the selected screen's item filled with `hover`.

use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::pressable,
};
use crate::theme::{ExtraColor, ExtraSpacing, IconSize};

/// `Nav item/*` height.
pub const NAV_ITEM_HEIGHT: f32 = 28.0;

pub struct NavItem<'a> {
  pub id: String,
  pub icon: Icon,
  pub label: &'a str,
  /// What the item counts, at its end (`24`, or `—` while it is not known yet).
  pub count: Option<&'a str>,
  pub selected: bool,
}

impl NavItem<'_> {
  pub fn on_click(self, handler: impl Fn() + Send + Sync + 'static) -> Element {
    let (color, label_style) = if self.selected {
      (PaletteColor::TextPrimary, TypographyStyle::Label)
    } else {
      (PaletteColor::TextSecondary, TypographyStyle::Description)
    };
    let row = Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(NAV_ITEM_HEIGHT))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .spacing(ExtraSpacing::Inline.role())
      .align_items(Alignment::Center)
      .child(icon(self.icon, IconSize::Nav, color.clone()))
      .child(
        Text::new(self.label)
          .variant(label_style)
          .color(color)
          .nowrap()
          .flex(1.0),
      )
      .with_children(
        self
          .count
          .map(|count| Text::new(count).variant(TypographyStyle::Caption).nowrap()),
      );
    let item = pressable(handler)
      .id(self.id)
      .describe("a11y_name", self.label)
      .describe("state", if self.selected { "selected" } else { "enabled" })
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(NAV_ITEM_HEIGHT))
      .rounded(RadiusSize::Md)
      .child(row);
    if self.selected {
      item.background(ExtraColor::Hover.role()).into()
    } else {
      item
        .hovered_style(Style::new().background(ExtraColor::Hover.role()))
        .into()
    }
  }
}
