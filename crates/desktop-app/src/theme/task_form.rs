//! The design values of New task and Edit task (`New task · filled · dark` `fJ4v9` and its states in the
//! layer `ZLTEI`) that the UI kit's roles do not carry, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{RadiusSize, SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::style;

/// Text of the task form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskFormText {
  /// 11/600 `text_primary`: the project's initial in its tile (`K` `C0mXJ` in `Row · Project` `lra40`).
  ProjectInitial,
}

impl TaskFormText {
  pub const ALL: [Self; 1] = [Self::ProjectInitial];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::ProjectInitial => "task_form_project_initial",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::ProjectInitial => style(11.0, FontWeight::SemiBold, 1.0, 0.0, palette.text_primary),
    }
  }
}

/// Gaps and paddings of the task form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskFormSpacing {
  /// 28: above the form, inside the content panel (`Scroll` `h8n8M`).
  PageTop,
  /// 40: under the form's last section, above the action bar (`Scroll` `h8n8M`).
  PageBottom,
}

impl TaskFormSpacing {
  pub const ALL: [Self; 2] = [Self::PageTop, Self::PageBottom];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::PageTop => "task_form_page_top",
      Self::PageBottom => "task_form_page_bottom",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::PageTop => 28.0,
      Self::PageBottom => 40.0,
    }
  }
}

/// Radii of the task form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskFormRadius {
  /// 5: the project's tile (`Avatar` `cBsT5` in `Row · Project` `lra40`).
  ProjectTile,
}

impl TaskFormRadius {
  pub const ALL: [Self; 1] = [Self::ProjectTile];

  pub fn role(self) -> RadiusSize {
    RadiusSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::ProjectTile => "task_form_project_tile",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::ProjectTile => 5.0,
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
  for radius in TaskFormRadius::ALL {
    theme.set_radius_value(radius.role(), radius.value());
  }
}
