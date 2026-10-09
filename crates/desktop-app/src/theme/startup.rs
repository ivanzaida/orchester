//! The design values of the startup screens (`Startup · Orchester is already open` `g0Kj1`, `Startup ·
//! Orchester can't open its data` `kMM56` and the variants board `ut8NI`) that the UI kit's roles do not
//! carry, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::{fonts::ICONS, typography::style};

/// Text of the startup screens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupText {
  /// The 20 glyph of the icon tile (`Glyph` `Br0GY`).
  TileIcon,
}

impl StartupText {
  pub const ALL: [Self; 1] = [Self::TileIcon];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::TileIcon => "startup_tile_icon",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::TileIcon => TextStyle {
        font_family: Arc::from(ICONS),
        ..style(20.0, FontWeight::Normal, 1.0, 0.0, palette.text_secondary)
      },
    }
  }
}

/// Gaps and paddings of the startup screens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupSpacing {
  /// 20: between the parts of a startup screen (`gap` of `g0Kj1`).
  Block,
  /// 40: inside the content panel around it (`Content · Startup` `RqhE7`).
  Panel,
}

impl StartupSpacing {
  pub const ALL: [Self; 2] = [Self::Block, Self::Panel];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Block => "startup_block",
      Self::Panel => "startup_panel",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Block => 20.0,
      Self::Panel => 40.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  StartupText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in StartupSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
