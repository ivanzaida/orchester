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
use crate::theme::{ExtraRadius, ExtraSpacing, LOCKED_SWITCH_OPACITY};

/// Track and knob sizes (`K8ItM`, knob `Y0aZS`).
const TRACK_WIDTH: f32 = 28.0;
const TRACK_HEIGHT: f32 = 16.0;
const KNOB_SIZE: f32 = 10.0;

pub struct Switch {
  id: &'static str,
  label: Arc<str>,
  on: bool,
  emphasized: bool,
}

impl Switch {
  pub fn new(id: &'static str, label: Arc<str>, on: bool) -> Self {
    Self {
      id,
      label,
      on,
      emphasized: false,
    }
  }

  /// The label in the text color and size of a dialog's body, a little further from the track: a
  /// switch a dialog's confirmation waits for (`tpwQD` in 02.Q `LMrbK`), or the state a setting's
  /// switch is in (`q43RMP` in `lW7Sp`).
  pub fn emphasized(mut self) -> Self {
    self.emphasized = true;
    self
  }

  /// The label and its distance from the track.
  fn label(&self) -> (Text, SpacingSize) {
    if self.emphasized {
      (
        Text::new(&self.label)
          .variant(TypographyStyle::FieldLabel)
          .color(PaletteColor::TextPrimary)
          .nowrap(),
        ExtraSpacing::Inline.role(),
      )
    } else {
      (
        Text::new(&self.label).variant(TypographyStyle::Caption).nowrap(),
        SpacingSize::Sm,
      )
    }
  }

  /// The switch and its label, faded together as one row, not focusable and not flippable: the
  /// value is fixed, and the caller says why (04.B `lW7Sp`).
  pub fn locked(self) -> Element {
    let (label, gap) = self.label();
    Row::new()
      .id(self.id)
      .spacing(gap)
      .align_items(Alignment::Center)
      .opacity(LOCKED_SWITCH_OPACITY)
      .cursor(CursorIcon::NotAllowed)
      .describe("checked", if self.on { "true" } else { "false" })
      .describe("state", "locked")
      .child(self.track())
      .child(label)
      .into()
  }

  /// The switch and its label; a press anywhere on them flips it.
  pub fn render(self, flip: impl Fn() + Send + Sync + 'static) -> Element {
    let switch = self.track();
    let (label, gap) = self.label();
    keyboard_stop(Button::empty().button(), Ring::Outer)
      .id(self.id)
      .spacing(gap)
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Sm)
      .cursor(CursorIcon::Pointer)
      .describe("checked", if self.on { "true" } else { "false" })
      .on_click(move |_| flip())
      .child(switch)
      .child(label)
      .into()
  }

  /// The track with its knob: off in the raised surface with a muted knob at the left (`K8ItM`),
  /// on in the accent with an inverse knob at the right (`E1Y9KK`).
  fn track(&self) -> Row {
    let (justify, track, border, knob) = if self.on {
      (
        Justify::End,
        PaletteColor::Accent,
        PaletteColor::Accent,
        PaletteColor::TextInverse,
      )
    } else {
      (
        Justify::Start,
        PaletteColor::SurfaceRaised,
        PaletteColor::Border,
        PaletteColor::TextSecondary,
      )
    };
    Row::new()
      .size(Dimension::Px(TRACK_WIDTH), Dimension::Px(TRACK_HEIGHT))
      .padding(ExtraSpacing::Hair.role())
      .align_items(Alignment::Center)
      .justify(justify)
      .background(track)
      .border_inside(BorderSize::Sm, border)
      .rounded(ExtraRadius::Pill.role())
      .child(
        Row::new()
          .id(format!("{}-knob", self.id))
          .size(Dimension::Px(KNOB_SIZE), Dimension::Px(KNOB_SIZE))
          .background(knob)
          .rounded(ExtraRadius::Pill.role()),
      )
  }
}
