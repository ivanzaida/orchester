//! The design values of the task page (ORC-100 `Task page · redesign` `C3wC8`, and the components of `Task page ·
//! components` `n70u8y`) that the UI kit's roles do not carry, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::{fonts::ICONS, typography::style};

/// Text of the task page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskText {
  /// 13/600 `text_primary`: a section's title on the Summary, a decision card's title (`Text` `K5NAZ` in `Heading ·
  /// Run` `Hz1jO`).
  SectionTitle,
  /// 13, 1.6: the task's brief, in `text_secondary` on the Summary (`Text` `fdcWw` in `Brief` `jP59S`).
  Brief,
  /// The 32 glyph of the Summary before the first run (`Qy6Ri` in `Xxrdy`).
  EmptyGlyph,
  /// 20/600 `text_primary`: *No runs yet* (`tg9zG`).
  EmptyTitle,
  /// 13 `text_secondary`, 1.6: what the Summary before the first run explains (`hD881`).
  EmptyText,
}

impl TaskText {
  pub const ALL: [Self; 5] = [
    Self::SectionTitle,
    Self::Brief,
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
      Self::Brief => "task_brief",
      Self::EmptyGlyph => "task_empty_glyph",
      Self::EmptyTitle => "task_empty_title",
      Self::EmptyText => "task_empty_text",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::SectionTitle => style(13.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::Brief => style(13.0, FontWeight::Normal, 1.6, 0.0, palette.text_secondary),
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
  /// 22: between the parts of a decision on the Summary, its card and what it rests on.
  Block,
  /// 28: under the Summary's last section.
  Foot,
  /// 20: over the header's first row (`Task header` `j8DakG`).
  Header,
  /// 28: the page's sides, the header's and the Summary's (`j8DakG`, `Body` `PSNRw`).
  Side,
  /// 28: between the sections of the Summary's column (`Main` `r1D4QA`).
  Sections,
  /// 40: between the Summary's column and its properties (`Body` `PSNRw`).
  Columns,
  /// 7: over and under a property's row (`Status` `u0BU0g` in `Properties` `Sq8ss`).
  PropertyRow,
  /// 9: over and under an acceptance criterion's row (`Criterion 1` `JDHDx`).
  CriterionRow,
}

impl TaskSpacing {
  pub const ALL: [Self; 8] = [
    Self::Block,
    Self::Foot,
    Self::Header,
    Self::Side,
    Self::Sections,
    Self::Columns,
    Self::PropertyRow,
    Self::CriterionRow,
  ];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Block => "task_block",
      Self::Foot => "task_foot",
      Self::Header => "task_header",
      Self::Side => "task_side",
      Self::Sections => "task_sections",
      Self::Columns => "task_columns",
      Self::PropertyRow => "task_property_row",
      Self::CriterionRow => "task_criterion_row",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Block => 22.0,
      Self::Header => 20.0,
      Self::Foot | Self::Side | Self::Sections => 28.0,
      Self::Columns => 40.0,
      Self::PropertyRow => 7.0,
      Self::CriterionRow => 9.0,
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
