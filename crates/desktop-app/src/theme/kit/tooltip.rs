//! Tooltip (`Tooltip` `b6q3OZ`, `ui::tooltip`): lurq's `ui::Tooltip`, a label in 11/500 `text_inverse` on
//! `text_primary`, radius 4, 4 over and under it and 8 at its sides, a shortcut 8 after it in Geist Mono 10 at 60 %
//! with no cap of its own; one with a note (`oL5mO`) 6 over and under its title, the note 2 under it in 11 regular. A
//! path or a command is in Geist Mono 11/500 (`C9Phsz`). A wrapping label keeps the one-line padding and leading. It
//! stands 6 from what it describes, with no arrow and no fade.

use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize, Theme, TypographyStyle},
  node::color::Color,
  ui::{Length, Tooltip, TooltipPart, TooltipState},
};

/// A shortcut with no cap of its own.
const NO_CAP: Color = Color::new(0, 0, 0, 0);

use crate::theme::{ExtraSpacing, ExtraTypography, SHORTCUT_OPACITY};

pub(super) fn install(theme: &Theme) {
  theme.patch_recipe::<Tooltip>(|rules| {
    rules
      .base(TooltipPart::Panel, |style| {
        style
          .background(PaletteColor::Foreground)
          .foreground(PaletteColor::PrimaryForeground)
          .radius(RadiusSize::Sm)
          .padding_x(SpacingSize::Sm)
          .padding_y(SpacingSize::Xs)
          .gap(SpacingSize::Sm)
      })
      .base(TooltipPart::Label, |style| {
        style.typography(TypographyStyle::FieldLabel)
      })
      .base(TooltipPart::Shortcut, |style| {
        style
          .typography(ExtraTypography::MonoSmall.role())
          .background(NO_CAP)
          .padding_x(0.0)
          .height(Length::Auto)
          .opacity(SHORTCUT_OPACITY)
      })
      .base(TooltipPart::Note, |style| {
        style
          .typography(TypographyStyle::Caption)
          .foreground(PaletteColor::PrimaryForeground)
          .padding_top(ExtraSpacing::Tight.role())
      })
      .state(TooltipState::Multiline, TooltipPart::Panel, |style| {
        style.padding_y(SpacingSize::Xs)
      })
      .state(TooltipState::Multiline, TooltipPart::Label, |style| {
        style.typography(TypographyStyle::FieldLabel)
      })
      .state(TooltipState::Note, TooltipPart::Panel, |style| {
        style.padding_y(ExtraSpacing::Snug.role())
      })
      .state(TooltipState::Mono, TooltipPart::Label, |style| {
        style.typography(ExtraTypography::MonoLabel.role())
      })
  });
}
