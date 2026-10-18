//! The design values of the task page (`Task · Summary · running · full page · dark` `lUXff` and the
//! components of `Task page · components` `n70u8y`) that the UI kit's roles do not carry, registered as
//! lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::{fonts::ICONS, typography::style};

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
  /// 15/600 `text_primary`, 1.6: a section's heading on the Details (`EFLic`, `NEOxs` in `UxsRH`).
  DetailsHeading,
  /// 16 `text_primary`, 1.6: the task's title on the Details (`cUorR`).
  DetailsTitle,
  /// 12 `text_primary`, 1.6: the description and a criterion on the Details (`RJxM7`, `T9YbPM`), the
  /// explanation of the creation version that was not recorded in `text_secondary` (`mN682` in `R3Wkvd`).
  DetailsBody,
  /// 11 `text_muted`, 1.6: where the requirements shown come from, what a side section holds (`FKZ6U`,
  /// `Fz663`).
  DetailsNote,
  /// 12/500, 1.6: that the creation version was not recorded, in `warning` (`gjOG6` in `R3Wkvd`).
  DetailsNoticeHeading,
  /// The 32 glyph of the Summary before the first run (`Qy6Ri` in `Xxrdy`).
  EmptyGlyph,
  /// 20/600 `text_primary`: *No runs yet* (`tg9zG`).
  EmptyTitle,
  /// 13 `text_secondary`, 1.6: what the Summary before the first run explains (`hD881`).
  EmptyText,
}

impl TaskText {
  pub const ALL: [Self; 12] = [
    Self::SectionTitle,
    Self::StepName,
    Self::RunTitle,
    Self::Brief,
    Self::DetailsHeading,
    Self::DetailsTitle,
    Self::DetailsBody,
    Self::DetailsNote,
    Self::DetailsNoticeHeading,
    Self::EmptyGlyph,
    Self::EmptyTitle,
    Self::EmptyText,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::SectionTitle => "task_section_title",
      Self::StepName => "task_step_name",
      Self::RunTitle => "task_run_title",
      Self::Brief => "task_brief",
      Self::DetailsHeading => "task_details_heading",
      Self::DetailsTitle => "task_details_title",
      Self::DetailsBody => "task_details_body",
      Self::DetailsNote => "task_details_note",
      Self::DetailsNoticeHeading => "task_details_notice_heading",
      Self::EmptyGlyph => "task_empty_glyph",
      Self::EmptyTitle => "task_empty_title",
      Self::EmptyText => "task_empty_text",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::SectionTitle => style(13.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::StepName => style(12.5, FontWeight::Medium, 1.3, 0.0, palette.text_primary),
      Self::RunTitle => style(12.5, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::Brief => style(13.0, FontWeight::Normal, 1.55, 0.0, palette.text_primary),
      Self::DetailsHeading => style(15.0, FontWeight::SemiBold, 1.6, 0.0, palette.text_primary),
      Self::DetailsTitle => style(16.0, FontWeight::Normal, 1.6, 0.0, palette.text_primary),
      Self::DetailsBody => style(12.0, FontWeight::Normal, 1.6, 0.0, palette.text_primary),
      Self::DetailsNote => style(11.0, FontWeight::Normal, 1.6, 0.0, palette.text_muted),
      Self::DetailsNoticeHeading => style(12.0, FontWeight::Medium, 1.6, 0.0, palette.warning),
      Self::EmptyGlyph => TextStyle {
        font_family: Arc::from(ICONS),
        ..style(32.0, FontWeight::Normal, 1.0, 0.0, palette.text_muted)
      },
      Self::EmptyTitle => style(20.0, FontWeight::SemiBold, 1.25, 0.0, palette.text_primary),
      Self::EmptyText => style(13.0, FontWeight::Normal, 1.6, 0.0, palette.text_secondary),
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
