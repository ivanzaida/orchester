//! Values of the Agent picker and the step's Agent field (`Agent picker` `Zn8BM` in K4 `CwLfL`; the field `Input`
//! `rMu3f` in P10 `HjgZ8`).

use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, TypographyStyle},
  layout::{
    scrollbar::ScrollBarStyle,
    text_style::{FontWeight, TextStyle},
  },
};

use super::typography::style;

/// The list's scrollbar (`Scrollbar` `f8jPq` in `List` `f2sV4`): 3 wide, radius 2, 4 from the list's side and 6 from
/// its top and bottom, over the rows.
const SCROLLBAR_WIDTH: f32 = 3.0;
const SCROLLBAR_RADIUS: f32 = 2.0;
const SCROLLBAR_EDGE: f32 = 4.0;
const SCROLLBAR_END: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentPickerText {
  /// 11 `text_muted`, 1.2: *Agent profiles* and *Tools* (`M27vF`, `q33vKe`).
  Group,
  /// 12/500 `text_primary`, 1.2: an agent's name in the list (`uLVIH`).
  Name,
  /// 11.5 `text_muted`, 1.2: what it runs on (`NOSNq`); why it is not ready in `warning` (`i11s7`).
  Runs,
  /// 12 `text_secondary`, 1.2: the foot's *New agent profile* and *Agents ↗* (`tP9tR`, `nmDoa`).
  Foot,
  /// 12.5/500 `text_primary`, 1.3: the field's profile name (`GyeX6` in `rMu3f`).
  FieldName,
  /// 12 `text_muted`, 1.3: what the field's profile runs on (`X7UJ9`).
  FieldRuns,
}

impl AgentPickerText {
  pub const ALL: [Self; 6] = [
    Self::Group,
    Self::Name,
    Self::Runs,
    Self::Foot,
    Self::FieldName,
    Self::FieldRuns,
  ];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Group => "agent_picker_group",
      Self::Name => "agent_picker_name",
      Self::Runs => "agent_picker_runs",
      Self::Foot => "agent_picker_foot",
      Self::FieldName => "agent_picker_field_name",
      Self::FieldRuns => "agent_picker_field_runs",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Group => style(11.0, FontWeight::Normal, 1.2, 0.0, palette.text_muted),
      Self::Name => style(12.0, FontWeight::Medium, 1.2, 0.0, palette.text_primary),
      Self::Runs => style(11.5, FontWeight::Normal, 1.2, 0.0, palette.text_muted),
      Self::Foot => style(12.0, FontWeight::Normal, 1.2, 0.0, palette.text_secondary),
      Self::FieldName => style(12.5, FontWeight::Medium, 1.3, 0.0, palette.text_primary),
      Self::FieldRuns => style(12.0, FontWeight::Normal, 1.3, 0.0, palette.text_muted),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  AgentPickerText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

/// `base`, the theme's scrollbar (its `border_strong` thumb), as the Agent picker's list draws it.
pub fn agent_picker_scrollbar(base: &ScrollBarStyle) -> ScrollBarStyle {
  ScrollBarStyle {
    width: SCROLLBAR_WIDTH,
    thumb_radius: SCROLLBAR_RADIUS,
    track_radius: SCROLLBAR_RADIUS,
    edge_inset: Some(SCROLLBAR_EDGE),
    end_inset: Some(SCROLLBAR_END),
    ..base.clone()
  }
}
