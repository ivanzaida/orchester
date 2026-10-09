//! The design values of the startup screens (`Startup · Orchester is already open` `g0Kj1`, `Startup ·
//! Orchester can't open its data` `kMM56` and the variants board `ut8NI`) that no shared role carries yet,
//! registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::{
  fonts::{ICONS, MONO},
  typography::style,
};

/// Text of the startup screens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupText {
  /// 13/500, 1.45, `text_primary`: what failed, under the heading (`Reason` `CocaS`).
  Reason,
  /// 13/600 `text_primary`: the heading of what to do (`To do title` `KdJzi`).
  Step,
  /// Geist Mono 11.5, 1.5, `text_primary`: a path, in its box or beside its key (`Command` `t30w57`).
  Path,
  /// Geist Mono 11, 1.45, `text_muted`: the log's path under the actions (`Footnote` `haOeA`).
  Log,
  /// The 20 glyph of the icon tile (`Glyph` `Br0GY`).
  TileIcon,
}

impl StartupText {
  pub const ALL: [Self; 5] = [Self::Reason, Self::Step, Self::Path, Self::Log, Self::TileIcon];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Reason => "startup_reason",
      Self::Step => "startup_step",
      Self::Path => "startup_path",
      Self::Log => "startup_log",
      Self::TileIcon => "startup_tile_icon",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Reason => style(13.0, FontWeight::Medium, 1.45, 0.0, palette.text_primary),
      Self::Step => style(13.0, FontWeight::SemiBold, 1.35, 0.0, palette.text_primary),
      Self::Path => TextStyle {
        font_family: Arc::from(MONO),
        ..style(11.5, FontWeight::Normal, 1.5, 0.0, palette.text_primary)
      },
      Self::Log => TextStyle {
        font_family: Arc::from(MONO),
        ..style(11.0, FontWeight::Normal, 1.45, 0.0, palette.text_muted)
      },
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
  /// 5: above and below a key and its value (`Detail · Data folder` `E197l`).
  Fact,
}

impl StartupSpacing {
  pub const ALL: [Self; 3] = [Self::Block, Self::Panel, Self::Fact];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Block => "startup_block",
      Self::Panel => "startup_panel",
      Self::Fact => "startup_fact",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Block => 20.0,
      Self::Panel => 40.0,
      Self::Fact => 5.0,
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
