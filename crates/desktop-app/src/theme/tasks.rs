//! The design values of the Tasks list (`Tasks · redesign` `Lw1Gb` in the new `orchester.pen`, ORC-99) and of the
//! Modules pages that share its frame, which the UI kit's roles do not carry, registered as lurq `Extra` roles.

use lurq::app::theme::{SpacingSize, Theme};

/// Gaps and paddings of the Tasks list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TasksSpacing {
  /// 40: inside the content panel at its sides on the Modules pages (`Content · Tasks` `fW8Mg`).
  Page,
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
  pub const ALL: [Self; 5] = [Self::Page, Self::Sides, Self::Stack, Self::GroupTop, Self::EmptyLift];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Page => "tasks_page",
      Self::Sides => "tasks_sides",
      Self::Stack => "tasks_stack",
      Self::GroupTop => "tasks_group_top",
      Self::EmptyLift => "tasks_empty_lift",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::Page => 40.0,
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
