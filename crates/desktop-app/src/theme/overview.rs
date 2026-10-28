//! The design values of the project's Overview (`Overview · redesign` `iRvT6`, frames 01–05) that the UI kit's roles do
//! not carry, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::style;

/// The line height the design's texts without one of their own take: Geist's own, about 16 for 12.5.
const NATURAL_LINE: f32 = 1.27;

/// Text of the Overview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverviewText {
  /// 20/600 `text_primary`, spaced -0.2: the project's name in the header (`Title` `gWdBN`).
  Name,
  /// 13/600 `text_primary`: a section's name (*At work*, `O63czf`).
  Section,
  /// 12.5/500 `text_primary`: the head of the digest and of the getting-started list (`r9fUI`, `TsAuA`).
  Head,
  /// 12.5 `text_primary`: a line's words (*Ready to merge · every step passed* `aAPvq`, in `text_secondary` there;
  /// *Retry webhooks with backoff* `tHE8y`).
  Line,
}

impl OverviewText {
  pub const ALL: [Self; 4] = [Self::Name, Self::Section, Self::Head, Self::Line];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Name => "overview_name",
      Self::Section => "overview_section",
      Self::Head => "overview_head",
      Self::Line => "overview_line",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Name => style(20.0, FontWeight::SemiBold, NATURAL_LINE, -0.2, palette.text_primary),
      Self::Section => style(13.0, FontWeight::SemiBold, NATURAL_LINE, 0.0, palette.text_primary),
      Self::Head => style(12.5, FontWeight::Medium, NATURAL_LINE, 0.0, palette.text_primary),
      Self::Line => style(12.5, FontWeight::Normal, NATURAL_LINE, 0.0, palette.text_primary),
    }
  }
}

/// Gaps and paddings of the Overview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverviewSpacing {
  /// 40: between the page's two columns (`Body` `yLIZK`).
  Columns,
}

impl OverviewSpacing {
  pub const ALL: [Self; 1] = [Self::Columns];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Columns => "overview_columns",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Columns => 40.0,
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
