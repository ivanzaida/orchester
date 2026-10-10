//! A primary button that opens a menu of what it can do (`Button/primary · menu` `Bm2gp`): a 14 icon, its
//! label in 12/500, a 1 × 14 split and a 14 chevron, all `on_accent`, 6 apart, 12 before the icon and 8
//! after the chevron, 28 high on `accent`, radius 6. A press opens the menu or closes it; the caller places
//! the menu at the button through the same `anchor`.

use std::sync::Arc;

use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Row, Text},
  core::{ElementRef, Signal},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::pressable,
};
use crate::theme::{ControlHeight, ExtraColor, ExtraSpacing, IconSize};

/// The split between the label and the chevron (`Split` `gsHDM`).
const SPLIT_HEIGHT: f32 = 14.0;

pub struct MenuButton {
  id: Arc<str>,
  glyph: Icon,
  label: Arc<str>,
  open: Signal<bool>,
  anchor: ElementRef,
}

impl MenuButton {
  /// The button `id` with `glyph` and `label`, opening the menu `open` shows at `anchor`.
  pub fn new(
    id: impl Into<Arc<str>>,
    (glyph, label): (Icon, impl Into<Arc<str>>),
    open: Signal<bool>,
    anchor: ElementRef,
  ) -> Self {
    Self {
      id: id.into(),
      glyph,
      label: label.into(),
      open,
      anchor,
    }
  }
}

impl From<MenuButton> for Element {
  fn from(button: MenuButton) -> Self {
    let on_accent = ExtraColor::OnAccent.role();
    let open = button.open.clone();
    let state = if button.open.get() { "open" } else { "closed" };
    pressable(move || open.update(|open| *open = !*open))
      .id(button.id.clone())
      .ref_element(button.anchor)
      .height(Dimension::Px(ControlHeight::Md.value()))
      .padding(Padding::new().left(SpacingSize::Md).right(SpacingSize::Sm))
      .spacing(ExtraSpacing::Snug.role())
      .background(PaletteColor::Accent)
      .rounded(RadiusSize::Md)
      .hovered_style(Style::new().background(PaletteColor::AccentHover))
      .describe("a11y_name", button.label.clone())
      .describe("state", state)
      .child(icon(button.glyph, IconSize::Md, on_accent.clone()))
      .child(
        Text::new(&button.label)
          .id(format!("{}-label", button.id))
          .variant(TypographyStyle::Button)
          .color(on_accent.clone())
          .nowrap(),
      )
      .child(
        Row::new()
          .width(Dimension::Px(1.0))
          .height(Dimension::Px(SPLIT_HEIGHT))
          .background(on_accent.clone()),
      )
      .child(icon(Icon::ChevronDown, IconSize::Md, on_accent))
      .into()
  }
}
