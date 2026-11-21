//! A primary button that opens a menu of what it can do (`Button/primary · menu` `Bm2gp`): a 14 icon, its
//! label in 12/500, a 1 × 14 split and a 14 chevron, all `on_accent`, 6 apart, 12 before the icon and 8
//! after the chevron, 28 high on `accent`, radius 6. A press opens the menu or closes it; the caller places
//! the menu at the button through the same `anchor`. lurq's `ui::Button` draws it, a primary one of the size
//! [`MENU`].

use std::sync::Arc;

use lurq::{
  components::Row,
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
  ui::{self, Styled},
};

use super::{
  button::ButtonKind,
  icon::{Icon, icon},
  pressable::Press,
};
use crate::theme::{ExtraColor, ExtraSpacing, IconSize, MENU_SPLIT_OPACITY};

/// Its size in the theme's recipe of lurq's `ui::Button` (`theme::kit::trigger`): a primary button's height, 12 before
/// the icon and 8 after the chevron.
pub const MENU: &str = "menu";

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
    let is_open = button.open.get();
    let chevron = if is_open { Icon::ChevronUp } else { Icon::ChevronDown };
    let end = Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(
        Row::new()
          .width(Dimension::Px(1.0))
          .height(Dimension::Px(SPLIT_HEIGHT))
          .background(on_accent.clone())
          .opacity(MENU_SPLIT_OPACITY),
      )
      .child(icon(chevron, IconSize::Md, on_accent.clone()));
    let trigger = ui::Button::new(&button.label)
      .id(button.id.clone())
      .ref_element(button.anchor)
      .variant(ButtonKind::Primary.name())
      .size(MENU)
      .tab_stop(false)
      .describe("a11y_name", button.label.clone())
      .describe("state", if is_open { "open" } else { "closed" })
      .leading(icon(button.glyph, IconSize::Md, on_accent))
      .trailing(end);
    Press::run(move || open.update(|open| *open = !*open))
      .on_ui(trigger)
      .into()
  }
}
