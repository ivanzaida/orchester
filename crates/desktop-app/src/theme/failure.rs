//! Values of the failure states (`Failure states · redesign` `SsC16` in the redesign file): a page that can't be read
//! (`Failure` `yT8TV` in `01` `urbxm`, with its details `QkFmK` in `01b` `K76n04`), a section and a row that can't be
//! read, a change that wasn't saved and a transient wait (`02 · Patterns` `N5if9`).
use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureText {
  /// 14/600 `text_primary`: what couldn't be read (`ShVBt` in `yT8TV`).
  Title,
  /// 12.5 `text_secondary`, 1.45: why, in plain words (`V6Cq7E`).
  Why,
  /// 12 `text_muted`, 1.45: what still works (`C9RwIN`).
  StillWorks,
  /// 12 `text_secondary`: the Details toggle (`dj5AL` in `zSoT8`).
  Toggle,
  /// Geist Mono 11.5 `text_secondary`, 1.5: the error's kind and message (`q7Bhd`, `CMHbe` in `e1uvnZ`).
  Detail,
  /// 12 `text_muted`: how many tries, and when the last was (`NMPh9` in `nrJJp`).
  Meta,
  /// 13/500 `text_primary`: a section's or a notice's title in a row (`Limits couldn't be read`, `The runner isn't
  /// connected` in `N5if9`).
  RowTitle,
  /// 12 `danger`: the header's save status after a change was not saved (`A change was not saved` in `w6gvw`).
  NotSaved,
}

impl FailureText {
  pub const ALL: [Self; 8] = [
    Self::Title,
    Self::Why,
    Self::StillWorks,
    Self::Toggle,
    Self::Detail,
    Self::Meta,
    Self::RowTitle,
    Self::NotSaved,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Title => "failure_title",
      Self::Why => "failure_why",
      Self::StillWorks => "failure_still_works",
      Self::Toggle => "failure_toggle",
      Self::Detail => "failure_detail",
      Self::Meta => "failure_meta",
      Self::RowTitle => "failure_row_title",
      Self::NotSaved => "failure_not_saved",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Title => style(14.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::Why => style(12.5, FontWeight::Normal, 1.45, 0.0, palette.text_secondary),
      Self::StillWorks => style(12.0, FontWeight::Normal, 1.45, 0.0, palette.text_muted),
      Self::Toggle => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary),
      Self::Detail => mono(style(11.5, FontWeight::Normal, 1.5, 0.0, palette.text_secondary)),
      Self::Meta => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::RowTitle => style(13.0, FontWeight::Medium, 1.3, 0.0, palette.text_primary),
      Self::NotSaved => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.danger),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureSpacing {
  /// 20: the card's sides (`yT8TV`).
  CardX,
  /// 18: over and under the card's parts (`yT8TV`).
  CardY,
  /// 14: between the card's top, actions and details (`yT8TV`).
  Parts,
  /// 40: the actions and details past the icon and its gap (`q6ymQ`, `QkFmK`).
  Indent,
  /// 10 over and under, 12 at the sides: the details' box (`e1uvnZ`).
  DetailY,
  /// 8 above the card in its pane (`dLaTg`).
  Above,
  /// 12: the icon to its words, and the details' box's sides (`R1h78`, `e1uvnZ`).
  Gap,
}

impl FailureSpacing {
  pub const ALL: [Self; 7] = [
    Self::CardX,
    Self::CardY,
    Self::Parts,
    Self::Indent,
    Self::DetailY,
    Self::Above,
    Self::Gap,
  ];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::CardX => "failure_card_x",
      Self::CardY => "failure_card_y",
      Self::Parts => "failure_parts",
      Self::Indent => "failure_indent",
      Self::DetailY => "failure_detail_y",
      Self::Above => "failure_above",
      Self::Gap => "failure_gap",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::CardX => 20.0,
      Self::CardY => 18.0,
      Self::Parts => 14.0,
      Self::Indent => 40.0,
      Self::DetailY => 10.0,
      Self::Above => 8.0,
      Self::Gap => 12.0,
    }
  }
}

pub(super) fn install(theme: &Theme) {
  for spacing in FailureSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  FailureText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
