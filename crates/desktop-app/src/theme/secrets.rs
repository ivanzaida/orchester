//! Values of Secrets (ccz8n), list DrLE6 and inspector V9Wz7d.
use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};
/// Pending field groups and primary actions, ZO3gD and fEyY0.
pub const SECRETS_PENDING_OPACITY: f32 = 0.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretsText {
  Lead,
  Name,
  InspectorName,
  Credential,
  ProjectInitial,
  Mask,
  InspectorTitle,
  ColumnHeading,
  ConfirmTitle,
  ValueHint,
  UsageHeading,
  EnvironmentEmpty,
}

impl SecretsText {
  pub const ALL: [Self; 12] = [
    Self::Lead,
    Self::Name,
    Self::InspectorName,
    Self::Credential,
    Self::ProjectInitial,
    Self::Mask,
    Self::InspectorTitle,
    Self::ColumnHeading,
    Self::ConfirmTitle,
    Self::ValueHint,
    Self::UsageHeading,
    Self::EnvironmentEmpty,
  ];
  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }
  const fn name(self) -> &'static str {
    match self {
      Self::Lead => "secrets_lead",
      Self::Name => "secrets_name",
      Self::InspectorName => "secrets_inspector_name",
      Self::Credential => "secrets_credential",
      Self::ProjectInitial => "secrets_project_initial",
      Self::Mask => "secrets_mask",
      Self::InspectorTitle => "secrets_inspector_title",
      Self::ColumnHeading => "secrets_column_heading",
      Self::ConfirmTitle => "secrets_confirm_title",
      Self::ValueHint => "secrets_value_hint",
      Self::UsageHeading => "secrets_usage_heading",
      Self::EnvironmentEmpty => "secrets_environment_empty",
    }
  }
  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Lead => style(13.0, FontWeight::Normal, 1.4, 0.0, palette.text_secondary),
      Self::Name => mono(style(12.5, FontWeight::Medium, 1.4, 0.0, palette.text_primary)),
      Self::InspectorName => mono(style(12.0, FontWeight::Normal, 1.4, 0.0, palette.text_secondary)),
      Self::Credential => mono(style(13.0, FontWeight::Normal, 1.4, 0.0, palette.text_primary)),
      Self::ProjectInitial => style(10.0, FontWeight::SemiBold, 1.0, 0.0, palette.text_secondary),
      Self::Mask => mono(style(14.0, FontWeight::Normal, 1.3, 0.0, palette.text_secondary)),
      Self::InspectorTitle => style(14.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::ColumnHeading => style(11.0, FontWeight::Medium, 1.3, 0.4, palette.text_muted),
      Self::ConfirmTitle => style(13.0, FontWeight::Medium, 1.4, 0.0, palette.text_primary),
      Self::ValueHint => style(11.0, FontWeight::Normal, 1.4, 0.0, palette.text_muted),
      Self::UsageHeading => style(11.0, FontWeight::Medium, 1.3, 0.0, palette.text_muted),
      Self::EnvironmentEmpty => style(12.5, FontWeight::Medium, 1.3, 0.0, palette.text_secondary),
    }
  }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretsSpacing {
  Page,
  Cell,
  Body,
}
impl SecretsSpacing {
  pub const ALL: [Self; 3] = [Self::Page, Self::Cell, Self::Body];
  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }
  const fn name(self) -> &'static str {
    match self {
      Self::Page => "secrets_page",
      Self::Cell => "secrets_cell",
      Self::Body => "secrets_body",
    }
  }
  pub const fn value(self) -> f32 {
    match self {
      Self::Page => 40.0,
      Self::Cell => 12.0,
      Self::Body => 14.0,
    }
  }
}
pub(super) fn install(theme: &Theme) {
  for spacing in SecretsSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  SecretsText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}
