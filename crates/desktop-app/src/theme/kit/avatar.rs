//! Avatars of letters (`ui::avatar`): a person (`Avatar` `x5rV0`) is their initials in 9/600 `text_primary` in a
//! round 22 face, `surface_raised` inside a `border_strong` stroke; the project of a credential editor (`YGy5i`) is
//! its initials in 10/600 `text_secondary` in a 20 face, radius 4, inside a `border` stroke.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, Theme},
  node::border::Border,
  ui::{Avatar, AvatarPart},
};

use crate::theme::{ExtraColor, ExtraRadius, ExtraTypography, SecretsText};

/// The size a person's avatar names (`x5rV0`).
pub const PERSON: &str = "person";
/// The size a credential's project names (`YGy5i`).
pub const PROJECT: &str = "project";

const PERSON_SIDE: f32 = 22.0;
const PROJECT_SIDE: f32 = 20.0;

pub(super) fn install(theme: &Theme) {
  theme.patch_recipe::<Avatar>(|rules| {
    rules
      .size(PERSON, AvatarPart::Face, |style| {
        style
          .square(PERSON_SIDE)
          .radius(ExtraRadius::Full.role())
          .background(PaletteColor::Card)
          .border(Border::inside(BorderSize::Sm, PaletteColor::Input))
      })
      .size(PERSON, AvatarPart::Initials, |style| {
        style
          .typography(ExtraTypography::Initials.role())
          .foreground(PaletteColor::Foreground)
      })
      .size(PROJECT, AvatarPart::Face, |style| {
        style
          .square(PROJECT_SIDE)
          .radius(RadiusSize::Sm)
          .background(PaletteColor::Card)
          .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      })
      .size(PROJECT, AvatarPart::Initials, |style| {
        style
          .typography(SecretsText::ProjectInitial.role())
          .foreground(ExtraColor::TextSecondary.role())
      })
  });
}
