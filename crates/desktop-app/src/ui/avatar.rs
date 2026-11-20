//! Avatars. A person (`Avatar` `x5rV0`) and a credential's project (`YGy5i`) are lurq's `ui::Avatar` of their
//! initials, in the theme's recipe (`theme::kit::avatar`). An agent (`Avatar` `HO4bJ` in `Agent message`
//! `dcd0m`) is its role's 13 `text_secondary` glyph in a 24 tile, radius 6, `surface_raised` inside a
//! `border` stroke.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, TypographyStyle},
  components::{Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{border::Border, dimension::Dimension},
  ui::Avatar,
};

use super::icon::{Icon, icon};
use crate::theme::{ExtraColor, IconSize, kit};

/// An agent's tile.
const AGENT_SIZE: f32 = 24.0;
/// Secrets tiles (I2uJV0, ptfRz, r0aRuu).
const CREDENTIAL_SIZE: f32 = 28.0;
const INSPECTOR_SIZE: f32 = 32.0;

fn tile(id: &str, size: f32) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Px(size))
    .height(Dimension::Px(size))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::Card)
}

/// A person by `name`, shown as its [`initials`]: lurq's `ui::Avatar` in the theme's recipe (`theme::kit::avatar`),
/// called by the name to tooling that reads the tree. Its letters' element id is `<id>-initials`.
pub fn person_avatar(id: &str, name: &str) -> Row {
  Row::new().child(
    Avatar::new(name)
      .initials(&initials(name))
      .size(kit::avatar::PERSON)
      .id(id.to_owned()),
  )
}

/// An agent by its role's glyph.
pub fn agent_avatar(id: &str, glyph: Icon) -> Row {
  tile(id, AGENT_SIZE)
    .rounded(RadiusSize::Md)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Base, ExtraColor::TextSecondary.role()))
}

/// The project of a credential editor (`YGy5i`) by its `name`, shown as its [`initials`].
pub fn credential_project_avatar(id: &str, name: &str) -> Row {
  Row::new().child(
    Avatar::new(name)
      .initials(&initials(name))
      .size(kit::avatar::PROJECT)
      .id(id.to_owned()),
  )
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
      ExtraColor::TextSecondary.role(),
    ))
}

/// The profile list's 28-pixel role tile (Agents / NA7sg / xV94H).
pub fn profile_avatar(id: &str, role: &str) -> Row {
  let letter: String = role.chars().take(1).flat_map(char::to_uppercase).collect();
  tile(id, 28.0)
    .rounded(RadiusSize::Md)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&letter)
        .variant(TypographyStyle::Label)
        .color(ExtraColor::TextSecondary.role()),
    )
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
