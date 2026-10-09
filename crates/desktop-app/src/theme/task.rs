//! The design values of the task page (`Task · Summary · running · full page · dark` `lUXff` and the
//! components of `Task page · components` `n70u8y`) that the UI kit's roles do not carry, registered as
//! lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::style;

/// Text of the task page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskText {
  /// 13/600 `text_primary`: a section's title on the Summary and in *This run* (`H8lxhg`, `UX8F3`).
  SectionTitle,
  /// 12.5/500 `text_primary`: a step's name in its progress row (`kHIsD` in `AfpbP`).
  StepName,
  /// 12.5/600 `text_primary`: a run's number in its row (`p2cZw` in `Db8k4`).
  RunTitle,
  /// 13 `text_primary`, 1.55: the task's brief (`X870e`).
  Brief,
}

impl TaskText {
  pub const ALL: [Self; 4] = [Self::SectionTitle, Self::StepName, Self::RunTitle, Self::Brief];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::SectionTitle => "task_section_title",
      Self::StepName => "task_step_name",
      Self::RunTitle => "task_run_title",
      Self::Brief => "task_brief",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::SectionTitle => style(13.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::StepName => style(12.5, FontWeight::Medium, 1.3, 0.0, palette.text_primary),
      Self::RunTitle => style(12.5, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::Brief => style(13.0, FontWeight::Normal, 1.55, 0.0, palette.text_primary),
    }
  }
}

/// Gaps and paddings of the task page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskSpacing {
  /// 22: between the Summary's sections (`gap` of `N3nD9i`).
  Block,
  /// 20: above the Summary's sections and inside *This run* (`N3nD9i`, `W73m1n`).
  Panel,
  /// 28: under the Summary's last section (`N3nD9i`).
  Foot,
}

impl TaskSpacing {
  pub const ALL: [Self; 3] = [Self::Block, Self::Panel, Self::Foot];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Block => "task_block",
      Self::Panel => "task_panel",
      Self::Foot => "task_foot",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Block => 22.0,
      Self::Panel => 20.0,
      Self::Foot => 28.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  TaskText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in TaskSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
