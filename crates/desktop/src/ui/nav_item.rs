//! Navigation item (`TSJpn`): icon and label. An item is either the selected screen or visibly
//! unavailable, because no other project screen exists yet.

use std::sync::Arc;

use lurq::{
  app::theme::{PaletteColor, RadiusSize, TypographyStyle},
  components::{Row, Text},
  layout::Alignment,
  node::{CursorIcon, Element, dimension::Dimension},
};

use super::icon::{Icon, IconSize, icon};
use crate::theme::ExtraSpacing;

/// Item heights: primary navigation (`TSJpn`), setup steps (`aaeeO`), footer entries (`wqkkE`).
const ITEM_HEIGHT: f32 = 38.0;
const STEP_HEIGHT: f32 = 44.0;
const COMPACT_HEIGHT: f32 = 30.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Density {
  Primary,
  /// Setup steps: taller, smaller label.
  Step,
  /// Secondary entries such as All projects: smaller icon and label.
  Secondary,
  /// Footer entries such as Project settings.
  Compact,
}

pub struct NavItem {
  icon: Icon,
  label: Arc<str>,
  selected: bool,
  density: Density,
  id: &'static str,
}

impl NavItem {
  /// An unavailable item unless marked `selected`.
  pub fn new(id: &'static str, icon: Icon, label: Arc<str>) -> Self {
    Self {
      icon,
      label,
      selected: false,
      density: Density::Primary,
      id,
    }
  }

  pub fn selected(mut self, selected: bool) -> Self {
    self.selected = selected;
    self
  }

  pub fn step(mut self) -> Self {
    self.density = Density::Step;
    self
  }

  pub fn secondary(mut self) -> Self {
    self.density = Density::Secondary;
    self
  }

  pub fn compact(mut self) -> Self {
    self.density = Density::Compact;
    self
  }
}

impl From<NavItem> for Element {
  fn from(item: NavItem) -> Self {
    let color = if item.selected {
      PaletteColor::TextPrimary
    } else {
      PaletteColor::TextMuted
    };
    let (height, icon_size, label_style) = match item.density {
      Density::Primary => (ITEM_HEIGHT, IconSize::Nav, TypographyStyle::Body),
      Density::Step => (STEP_HEIGHT, IconSize::Nav, TypographyStyle::FieldLabel),
      Density::Secondary => (ITEM_HEIGHT, IconSize::Button, TypographyStyle::FieldLabel),
      Density::Compact => (COMPACT_HEIGHT, IconSize::Button, TypographyStyle::FieldLabel),
    };
    let row = Row::new()
      .id(item.id)
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(height))
      .padding_horizontal(ExtraSpacing::Stack.role())
      .spacing(ExtraSpacing::Inline.role())
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Sm)
      .child(icon(item.icon, icon_size, color.clone()))
      .child(
        Text::new(&item.label)
          .variant(label_style)
          .color(color)
          .nowrap()
          .flex(1.0),
      );
    if item.selected {
      return row
        .background(PaletteColor::AccentMuted)
        .describe("state", "selected")
        .into();
    }
    row
      .describe("state", "unavailable")
      .cursor(CursorIcon::NotAllowed)
      .child(icon(Icon::Lock, IconSize::Small, PaletteColor::TextMuted))
      .into()
  }
}
