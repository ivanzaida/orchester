//! A sidebar navigation item (`Nav item/active` `q8fWpF`, `Nav item/default` `RAsOv`): icon and label,
//! and at its end a count in 11 `text_muted` (`Meta`), or in `warning` where it counts what waits for the person
//! (`Review` `H4DeIn` in `Zn8dR`), the selected screen's item filled with `hover`.

use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::opening,
};
use crate::{
  navigation::link::Open,
  theme::{ExtraColor, ExtraSpacing, IconSize},
};

/// `Nav item/*` height.
pub const NAV_ITEM_HEIGHT: f32 = 28.0;

pub struct NavItem<'a> {
  pub id: String,
  pub icon: Icon,
  pub label: &'a str,
  /// What the item counts, at its end (`24`, or `—` while it is not known yet).
  pub count: Option<&'a str>,
  /// The count is of what waits for the person: in `warning`.
  pub waiting: bool,
  pub selected: bool,
}

impl NavItem<'_> {
  /// The item, opening its screen with `open` by a browser's rules (`navigation::link`).
  pub fn opens(self, open: Open) -> Element {
    let (color, label_style) = if self.selected {
      (PaletteColor::TextPrimary, TypographyStyle::Label)
    } else {
      (PaletteColor::TextSecondary, TypographyStyle::Description)
    };
    let count_color = if self.waiting {
      PaletteColor::Warning
    } else {
      PaletteColor::TextMuted
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
      .with_children(self.count.map(|count| {
        Text::new(count)
          .variant(TypographyStyle::Caption)
          .color(count_color)
          .nowrap()
      }));
    let item = opening(open)
      .id(self.id)
      .describe("a11y_name", self.label)
      .describe("state", if self.selected { "selected" } else { "enabled" })
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(NAV_ITEM_HEIGHT))
      .rounded(RadiusSize::Md)
      .child(row);
    if self.selected {
      // The class marks the selected entry for the window's element tree and the tests.
      item.class("selected").background(ExtraColor::Hover.role()).into()
    } else {
      item
        .hovered_style(Style::new().background(ExtraColor::Hover.role()))
        .into()
    }
  }
}
