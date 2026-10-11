//! Avatars. A person (`Avatar` `x5rV0`) is their initials in 9/600 `text_primary` in a round 22 tile,
//! `surface_raised` inside a `border_strong` stroke. An agent (`Avatar` `HO4bJ` in `Agent message`
//! `dcd0m`) is its role's 13 `text_secondary` glyph in a 24 tile, radius 6, `surface_raised` inside a
//! `border` stroke.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize},
  components::{Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{border::Border, dimension::Dimension},
};

use super::icon::{Icon, icon};
use crate::theme::{ExtraRadius, ExtraTypography, IconSize, SecretsText};

/// A person's and an agent's tile.
const PERSON_SIZE: f32 = 22.0;
const AGENT_SIZE: f32 = 24.0;
/// Secrets tiles (I2uJV0, ptfRz, r0aRuu).
const CREDENTIAL_SIZE: f32 = 28.0;
const INSPECTOR_SIZE: f32 = 32.0;
const CREDENTIAL_PROJECT_SIZE: f32 = 20.0;

fn tile(id: &str, size: f32) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Px(size))
    .height(Dimension::Px(size))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
}

/// A person by their initials (see [`initials`]).
pub fn person_avatar(id: &str, initials: &str) -> Row {
  tile(id, PERSON_SIZE)
    .rounded(ExtraRadius::Full.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .child(
      Text::new(initials)
        .id(format!("{id}-label"))
        .variant(ExtraTypography::Initials.role())
        .nowrap(),
    )
}

/// An agent by its role's glyph.
pub fn agent_avatar(id: &str, glyph: Icon) -> Row {
  tile(id, AGENT_SIZE)
    .rounded(RadiusSize::Md)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Base, PaletteColor::TextSecondary))
}

/// The project of a credential editor (YGy5i): a 20 tile, radius 4, 10/600 initials.
pub fn credential_project_avatar(id: &str, name: &str) -> Row {
  tile(id, CREDENTIAL_PROJECT_SIZE)
    .rounded(RadiusSize::Sm)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(Text::new(&initials(name)).variant(SecretsText::ProjectInitial.role()))
}

/// Credential tiles of I2uJV0 (28) and ptfRz (32).
pub fn credential_avatar(id: &str, glyph: Icon, inspector: bool) -> Row {
  tile(id, if inspector { INSPECTOR_SIZE } else { CREDENTIAL_SIZE })
    .flex_shrink(0.0)
    .rounded(RadiusSize::Md)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(
      glyph,
      if inspector { IconSize::Nav } else { IconSize::Md },
      PaletteColor::TextSecondary,
    ))
}

/// The initials of `name`: the first letters of its first two words, in capitals (`Ada Lovelace` →
/// `AL`, `kontur` → `K`).
pub fn initials(name: &str) -> String {
  name
    .split_whitespace()
    .filter_map(|word| word.chars().next())
    .take(2)
    .flat_map(char::to_uppercase)
    .collect()
}
