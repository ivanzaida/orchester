//! The design values of the decision cards in the conversation (`Cards and rows inside the conversation`
//! `YhiGC`; `Question card` `tKYgO`, `Plan card` `Q5J0r`, `Approval card` `gwOaJ` in `Task page · components`
//! `n70u8y`) that the UI kit's roles do not carry, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::style;

/// Text of the cards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardsText {
  /// 12.5/600 `text_primary`: a question's or a plan's card title (`HSrmZ` in `tKYgO`, `tRrpK` in `Q5J0r`).
  Title,
  /// 13/600 `text_primary`: a tool request's card title (`WuwwR` in `gwOaJ`).
  ApprovalTitle,
  /// 12.5 `text_secondary`, 1.6: the lines of a plan (`vGWCj` in `Q5J0r`).
  PlanLines,
  /// 12.5/500 `text_primary`: what a decided request's row says (`EvLfP` in `E1En3`).
  Decided,
}

impl CardsText {
  pub const ALL: [Self; 4] = [Self::Title, Self::ApprovalTitle, Self::PlanLines, Self::Decided];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Title => "cards_title",
      Self::ApprovalTitle => "cards_approval_title",
      Self::PlanLines => "cards_plan_lines",
      Self::Decided => "cards_decided",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Title => style(12.5, FontWeight::SemiBold, 1.35, 0.0, palette.text_primary),
      Self::ApprovalTitle => style(13.0, FontWeight::SemiBold, 1.35, 0.0, palette.text_primary),
      Self::PlanLines => style(12.5, FontWeight::Normal, 1.6, 0.0, palette.text_secondary),
      Self::Decided => style(12.5, FontWeight::Medium, 1.35, 0.0, palette.text_primary),
    }
  }
}

/// Gaps and paddings of the cards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardsSpacing {
  /// 11: over and under a plan card's head (`torwy` in `Q5J0r`).
  PlanHead,
}

impl CardsSpacing {
  pub const ALL: [Self; 1] = [Self::PlanHead];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::PlanHead => "cards_plan_head",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::PlanHead => 11.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  CardsText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in CardsSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
