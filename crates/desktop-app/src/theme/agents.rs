//! Agents page padding from NPPl4/a7PAf and active task text from Rx5W5 in v7YbXO.
use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

pub fn page_spacing() -> SpacingSize {
  SpacingSize::extra("agents_page")
}

/// 12.5, normal, text_primary: the task on an active profile run (Rx5W5).
pub fn active_task_text() -> TypographyStyle {
  TypographyStyle::extra("agents_active_task")
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  std::iter::once((
    Arc::from("agents_active_task"),
    super::typography::style(12.5, FontWeight::Normal, 1.3, 0.0, palette.text_primary),
  ))
}

pub(super) fn install(theme: &Theme) {
  theme.set_spacing_value(page_spacing(), 40.0);
}
