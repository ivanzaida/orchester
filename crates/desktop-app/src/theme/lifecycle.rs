//! The design values of the window's lifecycle (layer "App lifecycle" `Ob1P7`: the closing panel
//! `u3Fw36`, `lxm8W`, `n5psM` and the interrupted runs of `JSRBO`, `AgD9q`) that the UI kit's roles do
//! not carry, registered as lurq `Extra` roles.

use std::sync::Arc;

use lurq::{
  app::theme::{SpacingSize, Theme, ThemePalette, TypographyStyle},
  layout::text_style::{FontWeight, TextStyle},
};

use super::typography::style;

/// The window's tabs while the closing panel is shown: in place, dimmed, and not reacting (`Tabs`
/// `L7gRf`).
pub const CLOSING_TABS_OPACITY: f32 = 0.4;

/// Text of the lifecycle screens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleText {
  /// 13/600 `text_primary`: a closing choice's title (`Title` `OA6xZ` in `Choice card` `zdSKi`).
  ChoiceTitle,
}

impl LifecycleText {
  pub const ALL: [Self; 1] = [Self::ChoiceTitle];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::ChoiceTitle => "lifecycle_choice_title",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::ChoiceTitle => style(13.0, FontWeight::SemiBold, 1.3, 0.0, palette.text_primary),
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
}

impl LifecycleSpacing {
  pub const ALL: [Self; 3] = [Self::PanelTop, Self::PanelSide, Self::RowEdge];

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::PanelTop => "lifecycle_panel_top",
      Self::PanelSide => "lifecycle_panel_side",
      Self::RowEdge => "lifecycle_row_edge",
    }
  }

  pub const fn value(self) -> f32 {
    match self {
      Self::PanelTop => 56.0,
      Self::PanelSide => 40.0,
      Self::RowEdge => 9.0,
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
