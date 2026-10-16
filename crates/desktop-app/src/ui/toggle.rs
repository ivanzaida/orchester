//! Controls that are on or off, beside an optional 12 `text_primary` label 8 from them; a press flips
//! them. They are form controls: Tab reaches them and Enter or Space flips them.
//! - Checkbox (`Checkbox/off` `q3JN1`, `Checkbox/on` `Una0j`): a 16 box, radius 4, `surface_input` in a `border_strong`
//!   stroke; on, `accent` with a 12 `on_accent` check.
//! - Radio (`Radio/off` `RchTU`, `Radio/on` `vsbEb`): a round 16 dot, `surface_input` in a `border_strong` stroke; on,
//!   `accent` with a round 6 `on_accent` center.
//! - Switch (`Switch/off` `H6LlDD`, `Switch/on` `XC5zU`): a round 28 × 16 track, 2 inside it, a round 12 knob; off,
//!   `border_strong` with a `text_secondary` knob at the start; on, `accent` with an `on_accent` knob at the end.
//!
//! A control whose value is fixed is faded to half and cannot be flipped; the caller says why. One settled by an
//! earlier step, which a review only lists, keeps its full strength.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Button, Row, Text},
  core::ElementRef,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};

use super::{
  icon::{Icon, icon},
  pressable::{Focus, inert, pressable},
};
use crate::theme::{DISABLED_OPACITY, ExtraColor, ExtraRadius, ExtraSpacing, IconSize};

/// A checkbox's box and a radio's dot (`ZZTTi`, `d4Pkdw`).
const MARK_SIZE: f32 = 16.0;
/// A radio's center (`usaPb`).
const RADIO_CENTER: f32 = 6.0;
/// A switch's track and knob (`H6LlDD`, `dY0v7`).
const TRACK_WIDTH: f32 = 28.0;
const TRACK_HEIGHT: f32 = 16.0;
const KNOB_SIZE: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToggleKind {
  Checkbox,
  Radio,
  Switch,
}

impl ToggleKind {
  const fn name(self) -> &'static str {
    match self {
      Self::Checkbox => "checkbox",
      Self::Radio => "radio",
      Self::Switch => "switch",
    }
  }
}

pub struct Toggle {
  id: Arc<str>,
  kind: ToggleKind,
  on: bool,
  /// What the control sets: its label beside it and its name for tools.
  label: Arc<str>,
  label_shown: bool,
  wrap_label: bool,
  /// The label's text style in its own color, where a design sets it apart from a form's label.
  label_style: Option<TypographyStyle>,
  change: Option<Arc<dyn Fn(bool) + Send + Sync>>,
  element: Option<ElementRef>,
  /// Room after the label, so the focus ring around the control clears its last letter.
  ring_room: bool,
  /// A fixed value decided by an earlier step, drawn at full strength.
  settled: bool,
}

impl Toggle {
  /// A control of `kind` that is `on`, setting what `label` says; the label shows beside it and names it
  /// for tools.
  pub fn new(id: impl Into<Arc<str>>, kind: ToggleKind, on: bool, label: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      kind,
      on,
      label: label.into(),
      label_shown: true,
      wrap_label: false,
      label_style: None,
      change: None,
      element: None,
      ring_room: false,
      settled: false,
    }
  }

  /// The label is said elsewhere, e.g. a setting's name at the row's start (`Inspector row/switch`
  /// `i4BnK6`): only tools read it here.
  pub fn hide_label(mut self) -> Self {
    self.label_shown = false;
    self
  }

  /// Hears the value a press asks for: the other one, or on for a radio. Without it the value is fixed:
  /// the control is faded and cannot be flipped.
  pub fn on_change(mut self, handler: impl Fn(bool) + Send + Sync + 'static) -> Self {
    self.change = Some(Arc::new(handler));
    self
  }

  /// The fixed value was decided by an earlier step and the control only shows it, at full strength: e.g. the
  /// selected snapshots a commit review lists (`kexy4`).
  pub fn settled(mut self) -> Self {
    self.settled = true;
    self
  }

  /// `element` refers to the control, e.g. so a screen can move the focus to it.
  pub fn element_ref(mut self, element: ElementRef) -> Self {
    self.element = Some(element);
    self
  }

  /// The label wraps onto more lines within the parent's width, the mark beside its first line: a label that may be
  /// long, such as one that names a folder.
  pub fn wrap_label(mut self) -> Self {
    self.wrap_label = true;
    self
  }

  /// The label in `style` and its color, e.g. an acknowledgement in an inspector's 11.5 `text_secondary`
  /// (`sVbfo` in Local integration).
  pub fn label_style(mut self, style: TypographyStyle) -> Self {
    self.label_style = Some(style);
    self
  }

  /// Room after the label for the focus ring, where the control stands among others in a card (`Z4HeE3`).
  pub fn ring_room(mut self) -> Self {
    self.ring_room = true;
    self
  }

  fn mark(&self) -> Row {
    let round = ExtraRadius::Full.role();
    let square = |size: f32| {
      Row::new()
        .width(Dimension::Px(size))
        .height(Dimension::Px(size))
        .align_items(Alignment::Center)
        .justify(Justify::Center)
    };
    let off = |shape: Row| {
      shape
        .background(PaletteColor::SurfaceInput)
        .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    };
    match (self.kind, self.on) {
      (ToggleKind::Checkbox, on) => check_mark(on),
      (ToggleKind::Radio, false) => off(square(MARK_SIZE)).rounded(round),
      (ToggleKind::Radio, true) => square(MARK_SIZE).rounded(round).background(PaletteColor::Accent).child(
        square(RADIO_CENTER)
          .rounded(round)
          .background(ExtraColor::OnAccent.role()),
      ),
      (ToggleKind::Switch, on) => {
        let (track, knob, side) = if on {
          (PaletteColor::Accent, ExtraColor::OnAccent.role(), Justify::End)
        } else {
          (PaletteColor::BorderStrong, PaletteColor::TextSecondary, Justify::Start)
        };
        Row::new()
          .width(Dimension::Px(TRACK_WIDTH))
          .height(Dimension::Px(TRACK_HEIGHT))
          .padding(ExtraSpacing::Tight.role())
          .align_items(Alignment::Center)
          .justify(side)
          .rounded(round)
          .background(track)
          .child(
            square(KNOB_SIZE)
              .rounded(round)
              .background(knob)
              .id(format!("{}-knob", self.id)),
          )
      }
    }
  }
}

/// A checkbox's mark alone (`Checkbox/off` `q3JN1`, `Checkbox/on` `Una0j`), for a row that is pressed as a
/// whole, e.g. a filter's option.
pub fn check_mark(on: bool) -> Row {
  let mark = Row::new()
    .width(Dimension::Px(MARK_SIZE))
    .height(Dimension::Px(MARK_SIZE))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(RadiusSize::Sm);
  if on {
    mark
      .background(PaletteColor::Accent)
      .child(icon(Icon::Check, IconSize::Sm, ExtraColor::OnAccent.role()))
  } else {
    mark
      .background(PaletteColor::SurfaceInput)
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
  }
}

impl From<Toggle> for Element {
  fn from(toggle: Toggle) -> Self {
    let mark = toggle.mark().id(format!("{}-mark", toggle.id));
    let label = toggle.label_shown.then(|| {
      let label = Text::new(&toggle.label).id(format!("{}-label", toggle.id));
      let label = match toggle.label_style {
        Some(style) => label.variant(style),
        None => label
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextPrimary),
      };
      if toggle.wrap_label {
        label.flex(1.0).min_width(Dimension::Px(0.0))
      } else {
        label.nowrap()
      }
    });
    let wraps = toggle.wrap_label;
    let wanted = toggle.kind == ToggleKind::Radio || !toggle.on;
    let button = match toggle.change {
      Some(change) => Focus::Form
        .apply(pressable(move || change(wanted)))
        .describe("state", "enabled"),
      None if toggle.settled => inert(Button::empty()).describe("state", "fixed"),
      None => inert(Button::empty())
        .opacity(DISABLED_OPACITY)
        .describe("state", "fixed"),
    };
    let button = match toggle.element {
      Some(element) => button.ref_element(element),
      None => button,
    };
    let button = if toggle.ring_room {
      button.padding_right(ExtraSpacing::Snug.role())
    } else {
      button
    };
    let button = if wraps {
      button.width(Dimension::Pct(100.0)).align_items(Alignment::Start)
    } else {
      button.align_items(Alignment::Center)
    };
    button
      .id(toggle.id)
      .spacing(SpacingSize::Sm)
      .rounded(RadiusSize::Sm)
      .describe("kind", toggle.kind.name())
      .describe("a11y_name", toggle.label.clone())
      .describe("checked", if toggle.on { "true" } else { "false" })
      .child(mark)
      .with_children(label)
      .into()
  }
}
