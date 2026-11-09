//! Values of a run's merge attempt page (ORC-363; `Merge attempt` `WJmCo` in 09b `PVZKL` of `Review · redesign`, and
//! 09e to 09l, and `Review - v2` 13a to 13i, row 7 `wbM48`): what stopped it, its checks, its attempts expanding into
//! their compact checks, and its facts.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::style;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptText {
  /// 12 `text_secondary`, 1.45: what stopped the attempt, or what happened, in its strip (`EmZqL` in 13c `zWlkp`).
  Explanation,
  /// 12 `text_muted`, 1.45: what the merge bar says under its title (`HGfP6`).
  BarBody,
  /// 20/600 `text_primary`, 1.4: the task's title in the header, which wraps (`TuXbv` in 13c `zWlkp`).
  Title,
}

impl AttemptText {
  pub const ALL: [Self; 3] = [Self::Explanation, Self::BarBody, Self::Title];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Explanation => "attempt_explanation",
      Self::BarBody => "attempt_bar_body",
      Self::Title => "attempt_title",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Explanation => style(12.0, FontWeight::Normal, 1.45, 0.0, palette.text_secondary),
      Self::BarBody => style(12.0, FontWeight::Normal, 1.45, 0.0, palette.text_muted),
      Self::Title => style(20.0, FontWeight::SemiBold, 1.4, 0.0, palette.text_primary),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptSpacing {
  /// 20: above the breadcrumb (`WJmCo`).
  Top,
  /// 11: over and under a check's row (`n8L7kz`).
  CheckY,
  /// 7: over and under a check's row in an expanded attempt (`Y3IjV5`), and a property's (`KWFBy`).
  CompactY,
  /// 28: an expanded attempt's checks in from the box's side (`D0SwP`).
  Indent,
}

impl AttemptSpacing {
  pub const ALL: [Self; 4] = [Self::Top, Self::CheckY, Self::CompactY, Self::Indent];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Top => "attempt_top",
      Self::CheckY => "attempt_check_y",
      Self::CompactY => "attempt_compact_y",
      Self::Indent => "attempt_indent",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Top => 20.0,
      Self::CheckY => 11.0,
      Self::CompactY => 7.0,
      Self::Indent => 28.0,
    }
  }
}

pub(super) fn install(theme: &Theme) {
  for spacing in AttemptSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  AttemptText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
