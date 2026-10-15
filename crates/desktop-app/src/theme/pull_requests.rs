//! The design values of the Pull requests page (`Pull requests · inbox · dark` `p9R7P` and its states) that
//! the UI kit's roles do not carry, registered as lurq `Extra` roles.

use lurq::app::theme::{SpacingSize, Theme};

/// Gaps and paddings of the Pull requests page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PullRequestsSpacing {
  /// 40: inside the content panel at its sides (`Content · Pull requests` `X6VN7J`).
  Page,
  /// 64: over the unavailable panel (`Center` `hK3t5`).
  Unavailable,
  /// 72: over an empty tab's glyph (`Empty` `r1gusH`).
  Empty,
}

impl PullRequestsSpacing {
  pub const ALL: [Self; 3] = [Self::Page, Self::Unavailable, Self::Empty];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Page => "pull_requests_page",
      Self::Unavailable => "pull_requests_unavailable",
      Self::Empty => "pull_requests_empty",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Page => 40.0,
      Self::Unavailable => 64.0,
      Self::Empty => 72.0,
    }
  }
}

pub(super) fn install(theme: &Theme) {
  for spacing in PullRequestsSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
