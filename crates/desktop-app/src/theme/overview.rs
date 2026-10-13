//! The design values of the project's Overview (`Overview · full page · dark` `q9mf0k` and its states in
//! `vgqpM`) that the UI kit's roles do not carry, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::style;

/// The line height the design's texts without one of their own take: Geist's own, about 14 for 11 and
/// 16 for 12.5.
const NATURAL_LINE: f32 = 1.27;

/// Text of the Overview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverviewText {
  /// 11.5/600 `warning`: what a *Needs you* card waits for (`Kind` `E4ZZK` in `rkhZ2`).
  Kind,
  /// 11/500 `text_secondary` on its natural line: a figure's name (`Label` `Xw3m5` in `Metric card`).
  FigureLabel,
  /// 11 `text_muted` on its natural line: what a figure counts (`Detail` `Bf2Nx` in `Metric card`).
  FigureDetail,
  /// 12.5 `text_primary` on its natural line: what happened to a run (`Text` `eH2AE` in `Activity row`).
  Event,
}

impl OverviewText {
  pub const ALL: [Self; 4] = [Self::Kind, Self::FigureLabel, Self::FigureDetail, Self::Event];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Kind => "overview_kind",
      Self::FigureLabel => "overview_figure_label",
      Self::FigureDetail => "overview_figure_detail",
      Self::Event => "overview_event",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Kind => style(11.5, FontWeight::SemiBold, 1.3, 0.0, palette.warning),
      Self::FigureLabel => style(11.0, FontWeight::Medium, NATURAL_LINE, 0.0, palette.text_secondary),
      Self::FigureDetail => style(11.0, FontWeight::Normal, NATURAL_LINE, 0.0, palette.text_muted),
      Self::Event => style(12.5, FontWeight::Normal, NATURAL_LINE, 0.0, palette.text_primary),
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
