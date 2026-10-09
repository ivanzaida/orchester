//! The design values of the decisions on a task's Summary (plan review `ciGbZ`, stopped at a limit
//! `oTzJZ`, waits after retries `DVuXu`, ready for review `F2LtFn`, merge states `kxMAN`) that the UI kit's
//! and the task page's roles do not carry, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::{palette::ExtraColor, typography::style};

/// Text of the decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionText {
  /// 13.5/600 `text_primary`: a decision card's title (`yPwsT` in `xHs82`, `Tyu6s` in `hLziL`).
  CardTitle,
  /// 14/600 `text_primary`: the title of the Ready card (`lKiJl` in `hyzq2`).
  ReadyTitle,
  /// 12/600 `text_primary`: a heading inside a card, who reported (`V4eb6`, `sm8lX`).
  Subhead,
  /// 13 `text_secondary`, 1.6: the plan as its agent wrote it (`Y1hz1`).
  Plan,
  /// 12.5 `text_secondary`, 1.45: an acceptance criterion in the plan's card (`y4265e`).
  Criterion,
  /// 13 `text_primary`, 1.55: what a step's agent reported (`RuLVp` in `cLh0O`).
  Report,
  /// 12/500 `accent_text`: a link to another tab (`bUBJC`, `hFIV4`).
  Link,
  /// Mono 11/600 `text_secondary`: how a file changed, `M`, `A`, `D` (`F1t7T` in `dAwTi`).
  ChangeKind,
}

impl DecisionText {
  pub const ALL: [Self; 8] = [
    Self::CardTitle,
    Self::ReadyTitle,
    Self::Subhead,
    Self::Plan,
    Self::Criterion,
    Self::Report,
    Self::Link,
    Self::ChangeKind,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::CardTitle => "decision_card_title",
      Self::ReadyTitle => "decision_ready_title",
      Self::Subhead => "decision_subhead",
      Self::Plan => "decision_plan",
      Self::Criterion => "decision_criterion",
      Self::Report => "decision_report",
      Self::Link => "decision_link",
      Self::ChangeKind => "decision_change_kind",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::CardTitle => style(13.5, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::ReadyTitle => style(14.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::Subhead => style(12.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::Plan => style(13.0, FontWeight::Normal, 1.6, 0.0, palette.text_secondary),
      Self::Criterion => style(12.5, FontWeight::Normal, 1.45, 0.0, palette.text_secondary),
      Self::Report => style(13.0, FontWeight::Normal, 1.55, 0.0, palette.text_primary),
      Self::Link => style(
        12.0,
        FontWeight::Medium,
        1.3,
        0.0,
        palette.get(ExtraColor::AccentText.role()),
      ),
      Self::ChangeKind => super::typography::mono(style(11.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_secondary)),
    }
  }
}

/// Gaps and paddings of the decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionSpacing {
  /// 18: the Ready card's sides (`hyzq2`).
  ReadySide,
  /// 20: between *What changed* and *Verification* (`Columns` `fwsep`).
  Columns,
}

impl DecisionSpacing {
  pub const ALL: [Self; 2] = [Self::ReadySide, Self::Columns];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::ReadySide => "decision_ready_side",
      Self::Columns => "decision_columns",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::ReadySide => 18.0,
      Self::Columns => 20.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  DecisionText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in DecisionSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
