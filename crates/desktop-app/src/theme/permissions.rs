//! Values of a run's permissions chip, its menu and its event lines (ORC-319; `Permissions chip` `BAck4`, `Permissions
//! menu` `WAMn1` in 15b `G1Vm1e`, `Event lines` `BaB32` in 15f `CimC4`) that the UI kit's roles do not carry.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::style;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionsText {
  /// 11.5 `text_secondary`, 1.3: what the chip says (`pmo77`); what the profile allows after it in `text_muted`
  /// (`T2MzK`).
  Chip,
  /// 11/500 `text_muted`, 1.3: a heading of the menu, *Run 9 · how Builder's requests are answered* (`XXJqP`).
  Heading,
  /// 11 `text_muted`, 1.3: an option's tag at its end, *narrows*, *only you* (`sImXd`).
  Tag,
  /// 11.5/500 `text_secondary`, 1.3: a similar rule's *Revoke* (`DyGig`).
  Revoke,
  /// 12/500 `text_secondary`, 1.3: an event line's action at its end, *Stop*, *Review…* (`BmZav`, `RDgYQ`).
  Action,
}

impl PermissionsText {
  pub const ALL: [Self; 5] = [Self::Chip, Self::Heading, Self::Tag, Self::Revoke, Self::Action];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Chip => "permissions_chip",
      Self::Heading => "permissions_heading",
      Self::Tag => "permissions_tag",
      Self::Revoke => "permissions_revoke",
      Self::Action => "permissions_action",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Chip => style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_secondary),
      Self::Heading => style(11.0, FontWeight::Medium, 1.3, 0.0, palette.text_muted),
      Self::Tag => style(11.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::Revoke => style(11.5, FontWeight::Medium, 1.3, 0.0, palette.text_secondary),
      Self::Action => style(12.0, FontWeight::Medium, 1.3, 0.0, palette.text_secondary),
    }
  }
}

/// Gaps and paddings of the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionsSpacing {
  /// 1: an option's glyph down to its name's first line (`Glyph` `d2EP7h`).
  Glyph,
}

impl PermissionsSpacing {
  pub const ALL: [Self; 1] = [Self::Glyph];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Glyph => "permissions_glyph",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Glyph => 1.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  PermissionsText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in PermissionsSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
