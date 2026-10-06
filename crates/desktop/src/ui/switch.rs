//! Switch (`K8ItM`): a small track with a knob, off at the left and on at the right, beside its
//! label. A form control: Tab reaches it and Enter or Space flips it (see `ui::pressable`). A locked
//! option keeps its note, faded with it, and says why it is fixed after the lock glyph ([`locked_reason`]).

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Button, Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, dimension::Dimension},
};

use super::{
  icon::{Icon, IconSize, icon},
  pressable::{Ring, keyboard_stop},
};
use crate::theme::{ExtraRadius, ExtraSpacing, ExtraTypography, LOCKED_SWITCH_OPACITY};

/// Track and knob sizes (`K8ItM`, knob `Y0aZS`).
const TRACK_WIDTH: f32 = 28.0;
const TRACK_HEIGHT: f32 = 16.0;
const KNOB_SIZE: f32 = 10.0;

pub struct Switch {
  id: Arc<str>,
  label: Arc<str>,
  label_visible: bool,
  on: bool,
  emphasized: bool,
  detail: Option<Arc<str>>,
  note: Option<Arc<str>>,
  /// Why a locked option is fixed: its id for tooling, and the words.
  reason: Option<(String, Arc<str>)>,
  /// Where the focus ring of an option is drawn.
  ring: Ring,
}

impl Switch {
  pub fn new(id: impl Into<Arc<str>>, label: Arc<str>, on: bool) -> Self {
    Self {
      id: id.into(),
      label,
      label_visible: true,
      on,
      emphasized: false,
      detail: None,
      note: None,
      reason: None,
      ring: Ring::Outer,
    }
  }

  /// The option's focus ring on its edge, for a switch in a panel that clips what reaches past it
  /// (the run's Approvals in its scrolling panel); the option keeps the ring and its gap inside it.
  pub const fn inset_ring(mut self) -> Self {
    self.ring = Ring::Inset;
    self
  }

  /// A control beside a separate setting label (`UCOuw`); retain its name in the MCP tree.
  pub fn hide_label(mut self) -> Self {
    self.label_visible = false;
    self
  }

  /// The label in the text color and size of a dialog's body, a little further from the track: a
  /// switch a dialog's confirmation waits for (`tpwQD` in 02.Q `LMrbK`), or the state a setting's
  /// switch is in (`q43RMP` in `lW7Sp`).
  pub fn emphasized(mut self) -> Self {
    self.emphasized = true;
    self
  }

  /// What the state means, after the label in the same row, which spaces the track, the label and
  /// the detail evenly (`k1QWD`).
  pub fn detail(mut self, detail: Arc<str>) -> Self {
    self.detail = Some(detail);
    self
  }

  /// A dialog option: the label as the option's title, with `note`, what choosing it does, wrapping
  /// under it, both beside the track at its top (`SmhER` in ORC-16 `R5y5x`). Pressing either flips
  /// the switch.
  pub fn note(mut self, note: Arc<str>) -> Self {
    self.note = Some(note);
    self
  }

  /// Why the option is fixed, under its note after the lock glyph, while it is [`Self::locked`].
  pub fn reason(mut self, id: String, reason: Arc<str>) -> Self {
    self.reason = Some((id, reason));
    self
  }

  /// The label and its distance from the track.
  fn label(&self) -> (Text, SpacingSize) {
    if self.emphasized {
      let gap = if self.detail.is_some() {
        ExtraSpacing::Stack.role()
      } else {
        ExtraSpacing::Inline.role()
      };
      (
        Text::new(&self.label)
          .variant(TypographyStyle::FieldLabel)
          .color(PaletteColor::TextPrimary)
          .nowrap(),
        gap,
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
    if let Some(note) = self.note.clone() {
      return self.locked_option(&note);
    }
    let (label, gap) = self.label();
    Row::new()
      .id(self.id.clone())
      .spacing(gap)
      .align_items(Alignment::Center)
      .opacity(LOCKED_SWITCH_OPACITY)
      .cursor(CursorIcon::NotAllowed)
      .describe("checked", if self.on { "true" } else { "false" })
      .describe("label", self.label.clone())
      .describe("state", "locked")
      .child(self.track())
      .with_children(self.label_visible.then_some(label))
      .with_children(self.detail.as_deref().map(detail))
      .into()
  }

  /// The switch and its label, then the detail if any; a press on the switch or its label flips it.
  pub fn render(self, flip: impl Fn() + Send + Sync + 'static) -> Element {
    if let Some(note) = &self.note {
      return self.option(note, flip);
    }
    let switch = self.track();
    let (label, gap) = self.label();
    let button = keyboard_stop(Button::empty().button(), Ring::Outer)
      .id(self.id.clone())
      .spacing(gap)
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Sm)
      .cursor(CursorIcon::Pointer)
      .describe("checked", if self.on { "true" } else { "false" })
      .describe("label", self.label.clone())
      .on_click(move |_| flip())
      .child(switch)
      .with_children(self.label_visible.then_some(label));
    match self.detail {
      // The button's outer focus ring reaches past the row.
      Some(text) => Row::new()
        .spacing(gap)
        .align_items(Alignment::Center)
        .overflow_visible()
        .child(button)
        .child(detail(&text))
        .into(),
      None => button.into(),
    }
  }

  /// An option's title over its note.
  fn copy(&self, note: &str) -> Column {
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xs)
      .child(
        Text::new(&self.label)
          .variant(ExtraTypography::NoticeHeading.role())
          .color(PaletteColor::TextPrimary),
      )
      .child(
        Text::new(note)
          .variant(ExtraTypography::CellLabel.role())
          .width(Dimension::Pct(100.0)),
      )
  }

  /// A locked option: the track, title and note faded where the option stands, then why it is fixed.
  fn locked_option(self, note: &str) -> Element {
    let row = Row::new();
    let row = match self.ring {
      Ring::Inset => row.padding(ExtraSpacing::FocusInset.role()),
      Ring::Outer => row,
    };
    row
      .id(self.id.clone())
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Start)
      .cursor(CursorIcon::NotAllowed)
      .describe("checked", if self.on { "true" } else { "false" })
      .describe("label", self.label.clone())
      .describe("state", "locked")
      .child(self.track().opacity(LOCKED_SWITCH_OPACITY))
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(SpacingSize::Xs)
          .child(self.copy(note).opacity(LOCKED_SWITCH_OPACITY))
          .with_children(
            self
              .reason
              .as_ref()
              .map(|(id, reason)| locked_reason(id.clone(), reason)),
          ),
      )
      .into()
  }

  fn option(&self, note: &str, flip: impl Fn() + Send + Sync + 'static) -> Element {
    let copy = self.copy(note).flex(1.0);
    let option = keyboard_stop(Button::empty().button(), self.ring);
    let option = match self.ring {
      Ring::Inset => option.padding(ExtraSpacing::FocusInset.role()),
      Ring::Outer => option,
    };
    option
      .id(self.id.clone())
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Start)
      .rounded(RadiusSize::Sm)
      .cursor(CursorIcon::Pointer)
      .describe("checked", if self.on { "true" } else { "false" })
      .on_click(move |_| flip())
      .child(self.track())
      .child(copy)
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

/// Why a switch is fixed, after the lock glyph, `id` for tooling.
pub fn locked_reason(id: String, reason: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Lock, IconSize::Small, PaletteColor::TextMuted))
    .child(
      Text::new(reason)
        .id(id)
        .variant(ExtraTypography::Hint.role())
        .color(PaletteColor::TextMuted)
        .flex(1.0),
    )
    .into()
}

/// The detail after a switch's label (`E42by` in `k1QWD`).
fn detail(text: &str) -> Text {
  Text::new(text).variant(TypographyStyle::Caption)
}
