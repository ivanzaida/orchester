//! Buttons that open a menu, lurq's `ui::Button` in the kit's look. The picker (`Picker` `ZQd2D`): 26 high on
//! `surface_input` in a `border` stroke, radius 6, 8 from its sides and 6 between its parts, the choice in 12
//! `text_primary`; its stroke `border_focus` while its menu is open (`LrZ5C` in `kadDh`) and `danger` when refused.
//! It draws no hover. The menu button (`Button/primary · menu` `Bm2gp`): a primary button 12 before its icon and 8
//! after its chevron.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, Theme, TypographyStyle},
  node::{CursorIcon, border::Border, color::Color},
  ui::{Button, ButtonPart, Recipe, Selector},
};

use super::plain::plain;
use crate::{
  theme::{ControlHeight, ExtraColor, ExtraSpacing},
  ui::{
    button::state,
    menu_button::MENU,
    picker::{OPEN, VARIANT},
  },
};

/// No veil over a pressed picker: the design draws no press.
const NO_VEIL: Color = Color::new(0, 0, 0, 0);

pub(super) fn install(theme: &Theme) {
  theme.patch_recipe::<Button>(|rules| menu(picker(rules)));
}

fn picker(rules: Recipe) -> Recipe {
  rules
    .variant(VARIANT, ButtonPart::Root, |style| {
      let mut style = style
        .height(ControlHeight::Picker.value())
        .padding_x(SpacingSize::Sm)
        .gap(ExtraSpacing::Snug.role())
        .radius(RadiusSize::Md)
        .background(ExtraColor::InputSurface.role())
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .cursor(CursorIcon::Pointer);
      style.transitions = Some(Vec::new());
      style.focus_ring = Some(false);
      style
    })
    .variant(VARIANT, ButtonPart::Label, |style| {
      style
        .typography(plain(TypographyStyle::Description))
        .foreground(PaletteColor::Foreground)
    })
    .when(Selector::new().variant(VARIANT).pressed(), ButtonPart::Root, |style| {
      style.veil(NO_VEIL)
    })
    .when(
      Selector::new().variant(VARIANT).state(OPEN),
      ButtonPart::Root,
      |style| style.border(Border::inside(BorderSize::Sm, PaletteColor::Ring)),
    )
    .when(
      Selector::new().variant(VARIANT).state(OPEN).state(state::INVALID),
      ButtonPart::Root,
      |style| style.border(Border::inside(BorderSize::Sm, PaletteColor::Destructive)),
    )
}

fn menu(rules: Recipe) -> Recipe {
  rules.size(MENU, ButtonPart::Root, |style| {
    style
      .height(ControlHeight::Md.value())
      .padding_start(SpacingSize::Md)
      .padding_end(SpacingSize::Sm)
  })
}
