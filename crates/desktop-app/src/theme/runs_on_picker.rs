//! Values of the compact *Runs on* picker (`Runs on picker` `ZcGGy` on `Pipelines · v2` `pCzIS`; K1 `lCApz`, K2
//! `rjPcV`).

use std::sync::Arc;

use lurq::{
  app::theme::{ThemePalette, TypographyStyle},
  layout::{
    scrollbar::ScrollBarStyle,
    text_style::{FontWeight, TextStyle},
  },
};

use super::typography::{mono, style};

/// A connection the picker cannot choose, at 40 % (`w58WS`, `miimJ`, `BMY4c` in `ZcGGy`; the brief calls it half).
pub const RUNS_ON_UNAVAILABLE_OPACITY: f32 = 0.4;

/// The model list's scrollbar (`Scrollbar` `KbY6m` in `BYfVi`): 3 wide, radius 2, 2 clear of the list's end and of its
/// top and bottom, over the rows.
const SCROLLBAR_WIDTH: f32 = 3.0;
const SCROLLBAR_RADIUS: f32 = 2.0;
const SCROLLBAR_INSET: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunsOnPickerText {
  /// 12/500 `text_primary`, 1.2: the connection's name on the line under the rail (`hyudy`).
  Name,
  /// 11.5 `text_muted`, 1.2: *Claude Code · Max plan* after it (`f2fcT`); why a connection cannot be chosen in
  /// `warning`.
  Caption,
  /// Geist Mono 10.5 `text_muted`, 1.2: *5h 41%* before the meter (`JLRnE`).
  Use,
  /// 11.5 `text_muted`, 1.2: an effort's segment (`k2S91`).
  Segment,
  /// 11.5/500 `text_primary`, 1.2: the chosen effort's segment (`LH6Q5`).
  SegmentChosen,
}

impl RunsOnPickerText {
  pub const ALL: [Self; 5] = [Self::Name, Self::Caption, Self::Use, Self::Segment, Self::SegmentChosen];

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Name => "runs_on_picker_name",
      Self::Caption => "runs_on_picker_caption",
      Self::Use => "runs_on_picker_use",
      Self::Segment => "runs_on_picker_segment",
      Self::SegmentChosen => "runs_on_picker_segment_chosen",
    }
  }

  fn style(self, palette: &ThemePalette) -> TextStyle {
    match self {
      Self::Name => style(12.0, FontWeight::Medium, 1.2, 0.0, palette.text_primary),
      Self::Caption => style(11.5, FontWeight::Normal, 1.2, 0.0, palette.text_muted),
      Self::Use => mono(style(10.5, FontWeight::Normal, 1.2, 0.0, palette.text_muted)),
      Self::Segment => style(11.5, FontWeight::Normal, 1.2, 0.0, palette.text_muted),
      Self::SegmentChosen => style(11.5, FontWeight::Medium, 1.2, 0.0, palette.text_primary),
    }
  }
}

pub(super) fn typography(palette: &ThemePalette) -> impl Iterator<Item = (Arc<str>, TextStyle)> {
  RunsOnPickerText::ALL
    .into_iter()
    .map(|text| (Arc::from(text.name()), text.style(palette)))
}

/// `base`, the theme's scrollbar (its `border_strong` thumb), as the compact picker's model list draws it.
pub fn runs_on_picker_scrollbar(base: &ScrollBarStyle) -> ScrollBarStyle {
  ScrollBarStyle {
    width: SCROLLBAR_WIDTH,
    thumb_radius: SCROLLBAR_RADIUS,
    track_radius: SCROLLBAR_RADIUS,
    edge_inset: Some(SCROLLBAR_INSET),
    end_inset: Some(SCROLLBAR_INSET),
    ..base.clone()
  }
}
