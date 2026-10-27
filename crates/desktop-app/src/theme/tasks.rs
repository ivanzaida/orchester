//! The design values of the Tasks list (`Tasks · redesign` `Lw1Gb` in the new `orchester.pen`, ORC-99), which the
//! Modules pages' groups and empty state share, that the UI kit's roles do not carry, registered as lurq `Extra`
//! roles.

use lurq::{
  app::theme::{SpacingSize, Theme},
  layout::scrollbar::ScrollBarStyle,
};

/// The list's scrollbar runs in the page's 28 at the panel's right edge, 10 in from it, clear of the rows (the design
/// draws none; the person asked for it away from the rows and near the edge).
const LIST_SCROLL_EDGE: f32 = 10.0;

/// `base`, the theme's scrollbar, placed for the Tasks list.
pub fn list_scrollbar(base: &ScrollBarStyle) -> ScrollBarStyle {
  ScrollBarStyle {
    edge_inset: Some(LIST_SCROLL_EDGE),
    ..base.clone()
  }
}

/// Gaps and paddings of the Tasks list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TasksSpacing {
  /// 28: inside the Tasks list's content panel at its sides (`Tasks` `klsF6`).
  Sides,
  /// 14: between the header, the views, the applied filters and the list (`klsF6`).
  Stack,
  /// 10: over a group's header line, inside its 34 (`Group · Needs you` `Sv7dZ`).
  GroupTop,
  /// 80: under the empty project's words, lifting them over the middle of the panel (`Empty` `GQZ6J`).
  EmptyLift,
}

impl TasksSpacing {
  pub const ALL: [Self; 4] = [Self::Sides, Self::Stack, Self::GroupTop, Self::EmptyLift];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Sides => "tasks_sides",
      Self::Stack => "tasks_stack",
      Self::GroupTop => "tasks_group_top",
      Self::EmptyLift => "tasks_empty_lift",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Sides => 28.0,
      Self::Stack => 14.0,
      Self::GroupTop => 10.0,
      Self::EmptyLift => 80.0,
    }
  }
}

pub(super) fn install(theme: &Theme) {
  for spacing in TasksSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
