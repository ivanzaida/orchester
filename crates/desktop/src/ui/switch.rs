//! Switch (`K8ItM`): a small track with a knob, off at the left and on at the right, beside its
//! label. A form control: Tab reaches it and Enter or Space flips it (see `ui::pressable`).

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Button, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, dimension::Dimension},
};

use super::pressable::{Ring, keyboard_stop};
use crate::theme::{ExtraRadius, ExtraSpacing};

/// Track and knob sizes (`K8ItM`, knob `Y0aZS`).
const TRACK_WIDTH: f32 = 28.0;
const TRACK_HEIGHT: f32 = 16.0;
const KNOB_SIZE: f32 = 10.0;

pub struct Switch {
  id: &'static str,
  label: Arc<str>,
  on: bool,
}

impl Switch {
  pub fn new(id: &'static str, label: Arc<str>, on: bool) -> Self {
    Self { id, label, on }
  }

  /// The switch and its label; a press anywhere on them flips it.
  pub fn render(self, flip: impl Fn() + Send + Sync + 'static) -> Element {
    let (justify, knob, track) = if self.on {
      (Justify::End, PaletteColor::TextPrimary, PaletteColor::AccentMuted)
    } else {
      (Justify::Start, PaletteColor::TextSecondary, PaletteColor::SurfaceRaised)
    };
    let switch = Row::new()
      .size(Dimension::Px(TRACK_WIDTH), Dimension::Px(TRACK_HEIGHT))
      .padding(ExtraSpacing::Hair.role())
      .align_items(Alignment::Center)
      .justify(justify)
      .background(track)
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(ExtraRadius::Pill.role())
      .child(
        Row::new()
          .size(Dimension::Px(KNOB_SIZE), Dimension::Px(KNOB_SIZE))
          .background(knob)
          .rounded(ExtraRadius::Pill.role()),
      );
    keyboard_stop(Button::empty().button(), Ring::Outer)
      .id(self.id)
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Sm)
      .cursor(CursorIcon::Pointer)
      .describe("checked", if self.on { "true" } else { "false" })
      .on_click(move |_| flip())
      .child(switch)
      .child(Text::new(&self.label).variant(TypographyStyle::Caption).nowrap())
      .into()
  }
}
