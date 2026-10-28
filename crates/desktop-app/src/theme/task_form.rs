//! The design values of New task and Edit task (the composer of `Task form · redesign` `HFXxX` in the new
//! `orchester.pen`: `S0TjN`, `lnN0y`, `jsySh`, `Qzvo0`) that the UI kit's roles do not carry, registered as lurq
//! `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::style;

/// Text of the task form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskFormText {
  /// 24/600 `text_primary`, tracking -0.4: the title typed on the page (`Title field` `klxTy`).
  Title,
  /// 14 `text_secondary`, 1.6: the outcome typed on the page (`Outcome field` `KNpPG`).
  Outcome,
  /// 13 `text_primary`, 1.3: an acceptance criterion in its row (`Criterion 1` `cScHP`).
  Criterion,
  /// 12 `text_muted`, 1.3: the count and the hint beside *Acceptance criteria* (`Count` `KrVl9`, `Hint` `ZndIB`).
  CriteriaHint,
  /// 11.5 `text_muted`, 1.3: a pipeline's steps and gates under its name in the Pipeline menu (`Meta` `H1NsU`).
  OptionMeta,
}

impl TaskFormText {
  pub const ALL: [Self; 5] = [
    Self::Title,
    Self::Outcome,
    Self::Criterion,
    Self::CriteriaHint,
    Self::OptionMeta,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Title => "task_form_title",
      Self::Outcome => "task_form_outcome",
      Self::Criterion => "task_form_criterion",
      Self::CriteriaHint => "task_form_criteria_hint",
      Self::OptionMeta => "task_form_option_meta",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Title => style(24.0, FontWeight::SemiBold, 1.3, -0.4, palette.text_primary),
      Self::Outcome => style(14.0, FontWeight::Normal, 1.6, 0.0, palette.text_secondary),
      Self::Criterion => style(13.0, FontWeight::Normal, 1.3, 0.0, palette.text_primary),
      Self::CriteriaHint => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
      Self::OptionMeta => style(11.5, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
    }
  }
}

/// Gaps and paddings of the task form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskFormSpacing {
  /// 28: above a full-page form, inside the content panel (other screens' pages keep it).
  PageTop,
  /// 40: under a full-page form's last section (other screens' pages keep it).
  PageBottom,
  /// 28: the composer's scroll area and the action bar at their sides (`Scroll` `h2N1Tl`, `Action bar` `IWxaO`).
  Side,
  /// 20: between the composer's parts (`Composer` `IxeaX`).
  Composer,
  /// 7: over and under a pipeline in the Pipeline menu (`Feature delivery v3` `I2BrOT`).
  MenuOption,
}

impl TaskFormSpacing {
  pub const ALL: [Self; 5] = [
    Self::PageTop,
    Self::PageBottom,
    Self::Side,
    Self::Composer,
    Self::MenuOption,
  ];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::PageTop => "task_form_page_top",
      Self::PageBottom => "task_form_page_bottom",
      Self::Side => "task_form_side",
      Self::Composer => "task_form_composer",
      Self::MenuOption => "task_form_menu_option",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::PageTop | Self::Side => 28.0,
      Self::PageBottom => 40.0,
      Self::Composer => 20.0,
      Self::MenuOption => 7.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  TaskFormText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in TaskFormSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
