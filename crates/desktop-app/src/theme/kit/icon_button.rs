//! Icon buttons (`Button/icon` `JJEzp`): lurq's `ui::Button`, its icon alone, in the variant `icon`. A 16
//! `text_secondary` glyph in a 28 square, radius 6, the `hover` fill under the pointer and `pressed` held down; a
//! smaller square and glyph for each size of [`IconButtonSize`]; while what it opened is open, the `pressed` fill and
//! a `text_primary` glyph.

use lurq::{
  app::theme::{PaletteColor, RadiusSize, Theme},
  node::{CursorIcon, color::Color},
  ui::{Button, ButtonPart, ButtonState, Recipe, Selector},
};

use crate::{
  theme::{ExtraColor, ExtraShadow},
  ui::icon_button::{ACTIVE, IconButtonSize, VARIANT},
};

/// No veil over the pressed fill.
const NO_VEIL: Color = Color::new(0, 0, 0, 0);

pub(super) fn install(theme: &Theme) {
  theme.patch_recipe::<Button>(|rules| {
    let rules = rules
      .variant(VARIANT, ButtonPart::Root, |style| {
        let mut style = style.radius(RadiusSize::Md).gap(0.0).cursor(CursorIcon::Pointer);
        style.transitions = Some(Vec::new());
        style
      })
      .when(Selector::new().variant(VARIANT).hovered(), ButtonPart::Root, |style| {
        style.background(ExtraColor::Hover.role())
      })
      .when(Selector::new().variant(VARIANT).pressed(), ButtonPart::Root, |style| {
        style.background(ExtraColor::Pressed.role()).veil(NO_VEIL)
      })
      .when(
        Selector::new().variant(VARIANT).focus_visible(),
        ButtonPart::Root,
        |style| {
          let mut style = style.shadow(ExtraShadow::FocusRing.role());
          style.focus_ring = Some(false);
          style
        },
      );
    let rules = IconButtonSize::ALL.into_iter().fold(rules, size);
    rules.when(
      Selector::new().variant(VARIANT).state(ACTIVE),
      ButtonPart::Root,
      |style| {
        style
          .background(ExtraColor::Pressed.role())
          .foreground(PaletteColor::Foreground)
      },
    )
  });
}

/// The square of a size, laid over lurq's square of an icon alone, and its glyph's side and color.
fn size(rules: Recipe, size: IconButtonSize) -> Recipe {
  let (width, height) = size.sides();
  let (glyph, color) = size.glyph();
  rules
    .when(
      Selector::new().size(size.name()).state(ButtonState::IconOnly),
      ButtonPart::Root,
      |style| style.width(width.value()).height(height.value()).foreground(color),
    )
    .size(size.name(), ButtonPart::Icon, |style| style.square(glyph.size()))
}
