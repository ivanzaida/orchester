//! Values of a confirmation modal (`Dialog` `LVCxf` in `04d · Bind an MCP server` `z0iFy`, which follows `05d · Delete
//! connection` `D3c8ht`): its title beside the tile, the label over its choice, the lines that say what it does, and
//! the *Esc cancels* words in its footer; its sides and the gap between its parts.
use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::style;

/// The dialog's width (`LVCxf`).
pub const DIALOG_WIDTH: f32 = 500.0;
/// The tile beside its title (`Tile` `jdSYt`).
pub const DIALOG_TILE: f32 = 32.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogText {
  /// 15/600 `text_primary`, 1.35: the question (`Title` `gqIuU`).
  Title,
  /// 11/500 `text_secondary`: the label over the dialog's choice (`Label` `y2I3c`).
  ChoiceLabel,
  /// 12.5 `text_secondary`, 1.45: a line of what it does (`Text` `RnZTC`).
  Line,
  /// 11.5 `text_muted`: what Esc does, beside its key (`Esc does` `lESYn`).
  Keys,
}

impl DialogText {
  pub const ALL: [Self; 4] = [Self::Title, Self::ChoiceLabel, Self::Line, Self::Keys];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Title => "dialog_title",
      Self::ChoiceLabel => "dialog_choice_label",
      Self::Line => "dialog_line",
      Self::Keys => "dialog_keys",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Title => style(15.0, FontWeight::SemiBold, 1.35, 0.0, palette.text_primary),
      Self::ChoiceLabel => style(11.0, FontWeight::Medium, 1.4, 0.0, palette.text_secondary),
      Self::Line => style(12.5, FontWeight::Normal, 1.45, 0.0, palette.text_secondary),
      Self::Keys => style(11.5, FontWeight::Normal, 1.4, 0.0, palette.text_muted),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogSpacing {
  /// 20: the dialog's sides and its body's top (`Body` `BuD1B`, `Footer` `APcrC`).
  Side,
  /// 14: between the body's parts (`BuD1B`).
  Parts,
}

impl DialogSpacing {
  pub const ALL: [Self; 2] = [Self::Side, Self::Parts];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Side => "dialog_side",
      Self::Parts => "dialog_parts",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Side => 20.0,
      Self::Parts => 14.0,
    }
  }
}

pub(super) fn install(theme: &Theme) {
  for spacing in DialogSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  DialogText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
