//! The design values of the Tasks list (`Tasks · redesign` `Lw1Gb` in the new `orchester.pen`, ORC-99), of its board
//! (`Tasks board · redesign` `J6gQo`, ORC-105) and of the Modules pages that share its frame, which the UI kit's roles
//! do not carry, registered as lurq `Extra` roles.

use std::time::Duration;

use lurq::{
  app::theme::{SpacingSize, Theme},
  layout::scrollbar::ScrollBarStyle,
};

/// The list's scrollbar runs in the page's 28 at the panel's right edge, 10 in from it, clear of the rows (the design
/// draws none; the person asked for it away from the rows and near the edge).
const LIST_SCROLL_EDGE: f32 = 10.0;

/// How long the tasks are read before their skeleton shows: a fast reading never flashes one (`Reading · skeleton`
/// `UWoGx`).
pub const SKELETON_DELAY: Duration = Duration::from_millis(300);
/// One gentle pulse of a skeleton, its bones dimmed to [`SKELETON_DIM`] at its middle; it stays still with reduced
/// motion. The pulse steps through [`SKELETON_STEPS`] poses: a frame is drawn only when the pose changes.
pub const SKELETON_PULSE: Duration = Duration::from_millis(1600);
pub const SKELETON_STEPS: u32 = 16;
pub const SKELETON_DIM: f32 = 0.55;
/// The list skeleton's lower lines fade (`05 · Loading` `R1nQlx`): its last group head, then its last four rows.
pub const SKELETON_FADE: [f32; 5] = [0.85, 0.7, 0.55, 0.4, 0.25];
/// The last card bone of a board column of three or more (`Card bone` `vR9Ky` in `17 · Loading` `Z1PMlo`).
pub const SKELETON_LAST_CARD: f32 = 0.55;
/// A card that is lifted away, or whose drop waits for the person, where it was (`KON-27` in `YhQ1y`, `KON-16` in
/// `yMirA`).
pub const HELD_CARD_OPACITY: f32 = 0.5;

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
