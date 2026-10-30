//! Values of the welcome screen, All projects and Add project (layer `Welcome & projects` `CGlZY`: `UQZRa`,
//! `S5HDKf`, the states `kwpO2`, `kkGWj`, `W5PIR`, `vJTcd` and the GitHub detection states `gh5wV`) that the UI
//! kit's roles do not carry, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

/// Text of the welcome screen, All projects and Add project.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectsText {
  /// Geist Mono 11 `text_muted`, tracked 0.6: *WELCOME TO ORCHESTER*, *PROJECT SETUP* (`NJQoY`, `SIvZm`).
  Eyebrow,
  /// 34/600, 1.12, tracked -0.8: the welcome's headline (`DAAGK`).
  WelcomeTitle,
  /// 14 `text_secondary`, 1.55: what the welcome says under it (`aFsI6`).
  WelcomeLead,
  /// Geist Mono 12 `text_muted`: a welcome step's number (`P0IME`).
  StepNumber,
  /// 13/600 `text_primary`: a block's title — a welcome step, the GitHub repository block, the removal
  /// question, *A home for this project's work* (`oL3s8`, `yFgXP`, `AoSrO`, `XWb1r`).
  Heading,
  /// 13 `text_secondary`, 1.45: what All projects lists, under its title (`dcMiE`).
  Sub,
  /// 11/500 `text_muted`: a column's heading in the project list (`xp4j3` in `L5sps`).
  ColumnHeading,
  /// 12/600 `text_primary`: a project's initial on its tile (`DuQhh`).
  Initial,
  /// 11.5 `text_secondary`, 1.45: why a listed project cannot be opened (`z596zF`).
  Hint,
  /// 12.5 `text_primary`, 1.45: why *Open project folder…* failed (`OswfE` in `FV5pQ`).
  ErrorLine,
  /// 12.5 `text_secondary`, 1.45: the team project offered above the list (`G1tpa` in `ZyS3T`, 09 `i5G19A`).
  Offer,
  /// Geist Mono 11.5 `text_primary`, 1.3: a folder, a remote, a host or scopes (`JQkW2`, `Eyhjb`, `UtQHq`).
  Path,
  /// 13/500 `text_primary`, 1.35: the setup step under way (`KKdxf`).
  Step,
  /// 13 `text_muted`, 1.35: a setup step not available yet (`XaPWn`).
  StepLocked,
  /// 12.5/600 `text_primary`: a section of *Change GitHub account or access* (`uLTTq`).
  SectionTitle,
  /// 12.5/500 `text_primary`: *Create a git repository here* in the GitHub block (`iUlxQ` in `OdlXb`).
  CreateTitle,
  /// 10.5/600 `text_muted`: the label over the first commit's choices, *FIRST COMMIT* (`T33T76`).
  ChoiceLabel,
  /// Geist Mono 11.5 `text_secondary`, 1.5: what git wrote when it failed (`gpVqf`).
  GitOutput,
  /// Geist Mono 11.5 `text_muted`, 1.3: the folders searched for an agent CLI (`qMnzd` in `U4QvYO`).
  SearchedFolders,
}

impl ProjectsText {
  pub const ALL: [Self; 19] = [
    Self::Eyebrow,
    Self::WelcomeTitle,
    Self::WelcomeLead,
    Self::StepNumber,
    Self::Heading,
    Self::Sub,
    Self::ColumnHeading,
    Self::Initial,
    Self::Hint,
    Self::ErrorLine,
    Self::Offer,
    Self::Path,
    Self::Step,
    Self::StepLocked,
    Self::SectionTitle,
    Self::CreateTitle,
    Self::ChoiceLabel,
    Self::GitOutput,
    Self::SearchedFolders,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Eyebrow => "projects_eyebrow",
      Self::WelcomeTitle => "projects_welcome_title",
      Self::WelcomeLead => "projects_welcome_lead",
      Self::StepNumber => "projects_step_number",
      Self::Heading => "projects_heading",
      Self::Sub => "projects_sub",
      Self::ColumnHeading => "projects_column_heading",
      Self::Initial => "projects_initial",
      Self::Hint => "projects_hint",
      Self::ErrorLine => "projects_error_line",
      Self::Offer => "projects_offer",
      Self::Path => "projects_path",
      Self::Step => "projects_step",
      Self::StepLocked => "projects_step_locked",
      Self::SectionTitle => "projects_section_title",
      Self::CreateTitle => "projects_create_title",
      Self::ChoiceLabel => "projects_choice_label",
      Self::GitOutput => "projects_git_output",
      Self::SearchedFolders => "projects_searched_folders",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    let sans = |size, weight, line_height, color| style(size, weight, line_height, 0.0, color);
    match self {
      Self::Eyebrow => mono(style(11.0, FontWeight::Normal, 1.3, 0.6, palette.text_muted)),
      Self::WelcomeTitle => style(34.0, FontWeight::SemiBold, 1.12, -0.8, palette.text_primary),
      Self::WelcomeLead => sans(14.0, FontWeight::Normal, 1.55, palette.text_secondary),
      Self::StepNumber => mono(sans(12.0, FontWeight::Normal, 1.3, palette.text_muted)),
      Self::Heading => sans(13.0, FontWeight::SemiBold, 1.3, palette.text_primary),
      Self::Sub => sans(13.0, FontWeight::Normal, 1.45, palette.text_secondary),
      Self::ColumnHeading => sans(11.0, FontWeight::Medium, 1.3, palette.text_muted),
      Self::Initial => sans(12.0, FontWeight::SemiBold, 1.3, palette.text_primary),
      Self::Hint => sans(11.5, FontWeight::Normal, 1.45, palette.text_secondary),
      Self::ErrorLine => sans(12.5, FontWeight::Normal, 1.45, palette.text_primary),
      Self::Offer => sans(12.5, FontWeight::Normal, 1.45, palette.text_secondary),
      Self::Path => mono(sans(11.5, FontWeight::Normal, 1.3, palette.text_primary)),
      Self::Step => sans(13.0, FontWeight::Medium, 1.35, palette.text_primary),
      Self::StepLocked => sans(13.0, FontWeight::Normal, 1.35, palette.text_muted),
      Self::SectionTitle => sans(12.5, FontWeight::SemiBold, 1.3, palette.text_primary),
      Self::CreateTitle => sans(12.5, FontWeight::Medium, 1.3, palette.text_primary),
      Self::ChoiceLabel => sans(10.5, FontWeight::SemiBold, 1.3, palette.text_muted),
      Self::GitOutput => mono(sans(11.5, FontWeight::Normal, 1.5, palette.text_secondary)),
      Self::SearchedFolders => mono(sans(11.5, FontWeight::Normal, 1.3, palette.text_muted)),
    }
  }
}

/// Gaps and paddings of the welcome screen, All projects and Add project.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectsSpacing {
  /// 20: between All projects' heading, tabs and list (`k4fycz`); a setup rail's bottom (`l9ZCVs`).
  Page,
  /// 40: the welcome's top and bottom (`p71bBF`), a welcome step's indent past the line (`RZyxY`), under the
  /// setup form (`r5sel`).
  Wide,
  /// 52: the removal question's indent, past a row's tile (`J4XaiO`).
  RowIndent,
  /// 56: the welcome's sides (`p71bBF`).
  WelcomeEdge,
  /// 72: between the welcome's introduction and its steps (`UvN35`).
  WelcomeColumns,
}

impl ProjectsSpacing {
  pub const ALL: [Self; 5] = [
    Self::Page,
    Self::Wide,
    Self::RowIndent,
    Self::WelcomeEdge,
    Self::WelcomeColumns,
  ];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Page => "projects_page",
      Self::Wide => "projects_wide",
      Self::RowIndent => "projects_row_indent",
      Self::WelcomeEdge => "projects_welcome_edge",
      Self::WelcomeColumns => "projects_welcome_columns",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Page => 20.0,
      Self::Wide => 40.0,
      Self::RowIndent => 52.0,
      Self::WelcomeEdge => 56.0,
      Self::WelcomeColumns => 72.0,
    }
  }
}

pub(super) fn install(theme: &Theme) {
  for spacing in ProjectsSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  ProjectsText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
