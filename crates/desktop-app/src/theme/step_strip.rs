//! Type roles of the step strip on an open step (`Step strip` in `Pipelines · v2` P2 `u2rYEH`, P13 `KRQ5J`).

use std::sync::Arc;

use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::{
    scrollbar::ScrollBarStyle,
    text_style::{FontWeight, TextStyle},
  },
  node::color::Color,
};

use super::{
  Mode,
  typography::{mono, style},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StripText {
  /// Geist Mono 11 `text_muted`, 1.2: a chip's number.
  Number,
  /// 12 `text_secondary`, 1.2: a chip's name.
  Name,
  /// 12 `text_primary`, 1.2: the open step's name.
  Current,
}

impl StripText {
  pub const ALL: [Self; 3] = [Self::Number, Self::Name, Self::Current];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Number => "step_strip_number",
      Self::Name => "step_strip_name",
      Self::Current => "step_strip_current",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Number => mono(style(11.0, FontWeight::Normal, 1.2, 0.0, palette.text_muted)),
      Self::Name => style(12.0, FontWeight::Normal, 1.2, 0.0, palette.text_secondary),
      Self::Current => style(12.0, FontWeight::Normal, 1.2, 0.0, palette.text_primary),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  StripText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StripSpacing {
  /// 7: a chip's sides (`Step 03` in `Step strip`).
  ChipSide,
}

impl StripSpacing {
  pub fn role(self) -> SpacingSize {
    SpacingSize::extra("step_strip_chip_side")
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::ChipSide => 7.0,
    }
  }
}

/// 4: a chip's corners and those of the frame round a parallel group (`Step 03`, `Parallel` in `Step strip`).
pub const STRIP_CHIP_RADIUS: f32 = 4.0;

pub fn install(theme: &Theme) {
  theme.set_spacing_value(StripSpacing::ChipSide.role(), StripSpacing::ChipSide.value());
  theme.set_radius_value(chip_radius(), STRIP_CHIP_RADIUS);
}

/// The role of a chip's corners.
pub fn chip_radius() -> RadiusSize {
  RadiusSize::extra("step_strip_chip")
}

/// `base`, the theme's scrollbar, as the strip's thin thumb under its chips draws it when the strip scrolls
/// (`Thumb` in P13 `KRQ5J`): 2 thick, 1 from the strip's bottom and 8 clear of its ends.
pub fn strip_scrollbar(base: &ScrollBarStyle) -> ScrollBarStyle {
  ScrollBarStyle {
    width: 2.0,
    thumb_radius: 1.0,
    track_radius: 1.0,
    edge_inset: Some(1.0),
    end_inset: Some(8.0),
    ..base.clone()
  }
}

/// Colors of the step strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StripColor {
  /// `surface_raised` fully transparent: where the fade under ‹ and › ends (`Fade` in P13 `KRQ5J`).
  FadeClear,
}

impl StripColor {
  pub fn role(self) -> PaletteColor {
    PaletteColor::extra("step_strip_fade_clear")
  }
}

pub(super) fn colors(mode: Mode) -> impl Iterator<Item = (Arc<str>, Color)> {
  let clear = match mode {
    Mode::Dark => "#20202300",
    Mode::Light => "#FFFFFF00",
  };
  [(Arc::<str>::from("step_strip_fade_clear"), Color::from_hex(clear))].into_iter()
}
