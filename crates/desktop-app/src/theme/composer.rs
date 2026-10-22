//! The design values of the conversation's composer (`Composer` `QQ2ZM`, `Send button/split` `wPMDa`, the
//! composer's modes in `YhiGC` and its queued state in `B67XdS`) that the UI kit's roles do not carry,
//! registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{PaletteColor, RadiusSize, Theme},
  node::color::Color,
};

use super::Mode;

/// Colors of the composer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComposerColor {
  /// `#00000033`: the line between a split send button's two parts (`Divider` `KkfI8` in `wPMDa`). The
  /// design draws it on the dark mode's light `accent`; on the light mode's dark one it is light.
  SplitDivider,
}

impl ComposerColor {
  pub const ALL: [Self; 1] = [Self::SplitDivider];

  pub fn role(self) -> PaletteColor {
    PaletteColor::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::SplitDivider => "composer_split_divider",
    }
  }

  fn color(self, mode: Mode) -> Color {
    match (self, mode) {
      (Self::SplitDivider, Mode::Dark) => Color::from_hex("#00000033"),
      (Self::SplitDivider, Mode::Light) => Color::from_hex("#FFFFFF33"),
    }
  }
}

/// Radii of the composer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComposerRadius {
  /// 8: the line over the box that says what the message answers or changes (`Context line` `B5qynx`).
  Context,
}

impl ComposerRadius {
  pub const ALL: [Self; 1] = [Self::Context];

  pub fn role(self) -> RadiusSize {
    RadiusSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Context => "composer_context",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Context => 8.0,
    }
  }
}

pub(super) fn colors(mode: Mode) -> impl Iterator<Item = (Arc<str>, Color)> {
  ComposerColor::ALL
    .into_iter()
    .map(move |color| (Arc::from(color.name()), color.color(mode)))
}

pub(super) fn install(theme: &Theme) {
  for radius in ComposerRadius::ALL {
    theme.set_radius_value(radius.role(), radius.value());
  }
}
