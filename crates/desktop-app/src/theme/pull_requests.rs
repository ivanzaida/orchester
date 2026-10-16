//! The design values of the Pull requests page (`Pull requests · inbox · dark` `p9R7P` and its states) and of
//! a pull request's page (`Pull request · Files changed` `mVTVX`) that the UI kit's roles do not carry,
//! registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

/// Gaps and paddings of the Pull requests page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PullRequestsSpacing {
  /// 40: inside the content panel at its sides (`Content · Pull requests` `X6VN7J`).
  Page,
  /// 64: over the unavailable panel (`Center` `hK3t5`).
  Unavailable,
  /// 72: over an empty tab's glyph (`Empty` `r1gusH`).
  Empty,
  /// 32: inside a pull request's page at its sides (`PR header` `nQXR1`, `Changes toolbar` `C4AKw`).
  Detail,
}

impl PullRequestsSpacing {
  pub const ALL: [Self; 4] = [Self::Page, Self::Unavailable, Self::Empty, Self::Detail];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Page => "pull_requests_page",
      Self::Unavailable => "pull_requests_unavailable",
      Self::Empty => "pull_requests_empty",
      Self::Detail => "pull_requests_detail",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Page => 40.0,
      Self::Unavailable => 64.0,
      Self::Empty => 72.0,
      Self::Detail => 32.0,
    }
  }
}

pub(super) fn install(theme: &Theme) {
  for spacing in PullRequestsSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}

/// Text of a pull request's page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PullRequestsText {
  /// The number beside the title: Geist Mono 17 `text_muted` (`Number` `eFZie`).
  TitleNumber,
  /// The count beside FILES: Geist Mono 10.5 `text_muted` (`Count` `SDcBY`).
  FileCount,
}

impl PullRequestsText {
  pub const ALL: [Self; 2] = [Self::TitleNumber, Self::FileCount];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::TitleNumber => "pull_requests_title_number",
      Self::FileCount => "pull_requests_file_count",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::TitleNumber => mono(style(17.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted)),
      Self::FileCount => mono(style(10.5, FontWeight::Normal, 1.35, 0.0, palette.text_muted)),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  PullRequestsText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
