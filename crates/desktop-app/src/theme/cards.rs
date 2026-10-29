//! The design values of the decision cards in the conversation (`Cards and rows inside the conversation`
//! `YhiGC`; `Question card` `tKYgO`, `Plan card` `Q5J0r`, `Approval card` `gwOaJ` in `Task page · components`
//! `n70u8y`; the inline decision card and its answered line of `Conversation cards · redesign` `p2o1w`) that the
//! UI kit's roles do not carry, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::style;

/// The answers an answered card did not choose, dimmed (`AAgL1`, `F0nR9` in `hm5wE`).
pub const NOT_CHOSEN_OPACITY: f32 = 0.5;

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
  /// 12.5 `text_primary`, 1.5: what a question asks, in its box in the Needs you panel (`l53lO` in `Z4HeE3`).
  Asked,
  /// 13 `text_secondary`, 1.55: the agent's own words in an inline decision card (`XxKPM` in `yYTFc`).
  Context,
  /// 15/600 `text_primary`, 1.4, tracking −0.1: the decision an inline card asks for (`a6Iu4` in `yYTFc`).
  Decision,
  /// 12/500 `text_secondary`, 1.3: *Show the whole question · 9 lines* (`O9yqiA` in `v1gom`).
  Whole,
  /// 12 `text_muted`, 1.3: the meta line under a card (`zJE5E` in `uEaZg`), the question and time of an answered
  /// line (`Qlch8` in `F16aNE`), the footer of an opened card (`NgTqU` in `hm5wE`).
  Quiet,
  /// 12 `text_secondary`, 1.3: who answered, at the start of an answered line (`c67tEB` in `F16aNE`).
  Lead,
  /// 12/500 `text_primary`, 1.3: what was chosen, in an answered line (`sGpQy` in `F16aNE`).
  Chosen,
}

impl CardsText {
  pub const ALL: [Self; 11] = [
    Self::Title,
    Self::ApprovalTitle,
    Self::PlanLines,
    Self::Decided,
    Self::Asked,
    Self::Context,
    Self::Decision,
    Self::Whole,
    Self::Quiet,
    Self::Lead,
    Self::Chosen,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Title => "cards_title",
      Self::ApprovalTitle => "cards_approval_title",
      Self::PlanLines => "cards_plan_lines",
      Self::Decided => "cards_decided",
      Self::Asked => "cards_asked",
      Self::Context => "cards_context",
      Self::Decision => "cards_decision",
      Self::Whole => "cards_whole",
      Self::Quiet => "cards_quiet",
      Self::Lead => "cards_lead",
      Self::Chosen => "cards_chosen",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Title => style(12.5, FontWeight::SemiBold, 1.35, 0.0, palette.text_primary),
      Self::ApprovalTitle => style(13.0, FontWeight::SemiBold, 1.35, 0.0, palette.text_primary),
      Self::PlanLines => style(12.5, FontWeight::Normal, 1.6, 0.0, palette.text_secondary),
      Self::Decided => style(12.5, FontWeight::Medium, 1.35, 0.0, palette.text_primary),
      Self::Asked => style(12.5, FontWeight::Normal, 1.5, 0.0, palette.text_primary),
      Self::Context => style(13.0, FontWeight::Normal, 1.55, 0.0, palette.text_secondary),
      Self::Decision => style(15.0, FontWeight::SemiBold, 1.4, -0.1, palette.text_primary),
      Self::Whole => style(12.0, FontWeight::Medium, 1.3, 0.0, palette.text_secondary),
      Self::Quiet => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::Lead => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary),
      Self::Chosen => style(12.0, FontWeight::Medium, 1.3, 0.0, palette.text_primary),
    }
  }
}

/// Gaps and paddings of the cards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardsSpacing {
  /// 11: over and under a plan card's head (`torwy` in `Q5J0r`).
  PlanHead,
  /// 18: over and under the inside of an inline decision card (`yYTFc`).
  InlineEnds,
  /// 20: at the sides of the inside of an inline decision card (`yYTFc`).
  InlineSides,
}

impl CardsSpacing {
  pub const ALL: [Self; 3] = [Self::PlanHead, Self::InlineEnds, Self::InlineSides];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::PlanHead => "cards_plan_head",
      Self::InlineEnds => "cards_inline_ends",
      Self::InlineSides => "cards_inline_sides",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::PlanHead => 11.0,
      Self::InlineEnds => 18.0,
      Self::InlineSides => 20.0,
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
