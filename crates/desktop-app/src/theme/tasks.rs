//! The design values of the Tasks list (`Tasks · full page · dark` `C2E5Po` and its states) that the UI
//! kit's roles do not carry, registered as lurq `Extra` roles.

use lurq::app::theme::{SpacingSize, Theme};

/// Gaps and paddings of the Tasks list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TasksSpacing {
  /// 40: inside the content panel at its sides (`Content · Tasks` `fW8Mg`).
  Page,
}

impl TasksSpacing {
  pub const ALL: [Self; 1] = [Self::Page];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Page => "tasks_page",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Page => 40.0,
    }
  }
}

pub(super) fn install(theme: &Theme) {
  for spacing in TasksSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
