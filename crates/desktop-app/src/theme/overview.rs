//! The design values of the project's Overview (`Overview · full page · dark` `q9mf0k` and its states in
//! `vgqpM`) that the UI kit's roles do not carry, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::style;

/// Text of the Overview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverviewText {
  /// 11.5/600 `warning`: what a *Needs you* card waits for (`Kind` `E4ZZK` in `rkhZ2`).
  Kind,
}

impl OverviewText {
  pub const ALL: [Self; 1] = [Self::Kind];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Kind => "overview_kind",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Kind => style(11.5, FontWeight::SemiBold, 1.3, 0.0, palette.warning),
    }
  }
}

/// Gaps and paddings of the Overview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverviewSpacing {
  /// 40: inside the content panel at its sides (`Content · Overview` `fiEe3`).
  Page,
  /// 22: above and below a reading line in a section (`Reading` `X59I5`).
  Reading,
}

impl OverviewSpacing {
  pub const ALL: [Self; 2] = [Self::Page, Self::Reading];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Page => "overview_page",
      Self::Reading => "overview_reading",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Page => 40.0,
      Self::Reading => 22.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  OverviewText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in OverviewSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
