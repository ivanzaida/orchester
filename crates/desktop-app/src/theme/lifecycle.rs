//! The design values of the window's lifecycle (layer "App lifecycle" `Ob1P7`: the closing panel
//! `u3Fw36`, `lxm8W`, `n5psM` and the interrupted runs of `JSRBO`, `AgD9q`) that the UI kit's roles do
//! not carry, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::{mono, style};

/// The window's tabs while the closing panel is shown: in place, dimmed, and not reacting (`Tabs`
/// `L7gRf`).
pub const CLOSING_TABS_OPACITY: f32 = 0.4;

/// A closing row's 12 text on the design's 16 line.
const ROW_LINE: f32 = 16.0 / 12.0;

/// Text of the lifecycle screens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleText {
  /// 13/600 `text_primary`: a closing choice's title (`Title` `OA6xZ` in `Choice card` `zdSKi`).
  ChoiceTitle,
  /// Geist Mono 12 `text_muted`, the design's own line (16): a closing row's task key (`Key` `O8L7JR`).
  RowKey,
  /// 12 `text_secondary`, the design's own line (16): a closing row's step, status and last cell (`Step`
  /// `DDIuo`, `Status label` `xZVMt`, `Last` `i0xS0`).
  RowText,
  /// 13 `text_primary`, 1.3: the step an interrupted run was interrupted during (`JSRBO`).
  Cell,
  /// 12/500 `text_primary`, 1.3: *View changes* under an interrupted run's worktree (`JSRBO`).
  Link,
}

impl LifecycleText {
  pub const ALL: [Self; 5] = [Self::ChoiceTitle, Self::RowKey, Self::RowText, Self::Cell, Self::Link];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::ChoiceTitle => "lifecycle_choice_title",
      Self::RowKey => "lifecycle_row_key",
      Self::RowText => "lifecycle_row_text",
      Self::Cell => "lifecycle_cell",
      Self::Link => "lifecycle_link",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::ChoiceTitle => style(13.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
      Self::RowKey => mono(style(12.0, FontWeight::Normal, ROW_LINE, 0.0, palette.text_muted)),
      Self::RowText => style(12.0, FontWeight::Normal, ROW_LINE, 0.0, palette.text_secondary),
      Self::Cell => style(13.0, FontWeight::Normal, 1.3, 0.0, palette.text_primary),
      Self::Link => style(12.0, FontWeight::Medium, 1.3, 0.0, palette.text_primary),
    }
  }
}

/// Gaps and paddings of the lifecycle screens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleSpacing {
  /// 56: above the closing panel in the content panel (`Content · Closing` `Gc2AK`).
  PanelTop,
  /// 40: beside the closing panel in the content panel (`Gc2AK`).
  PanelSide,
  /// 9: above and below a closing row's task (`Closing run row` `y6v6zN`).
  RowEdge,
  /// 40: the interrupted runs' notice from the sides of the Overview's content panel (`JSRBO`).
  NoticeSide,
}

impl LifecycleSpacing {
  pub const ALL: [Self; 4] = [Self::PanelTop, Self::PanelSide, Self::RowEdge, Self::NoticeSide];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::PanelTop => "lifecycle_panel_top",
      Self::PanelSide => "lifecycle_panel_side",
      Self::RowEdge => "lifecycle_row_edge",
      Self::NoticeSide => "lifecycle_notice_side",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::PanelTop => 56.0,
      Self::PanelSide => 40.0,
      Self::RowEdge => 9.0,
      Self::NoticeSide => 40.0,
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  LifecycleText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

pub(super) fn install(theme: &Theme) {
  for spacing in LifecycleSpacing::ALL {
    theme.set_spacing_value(spacing.role(), spacing.value());
  }
}
