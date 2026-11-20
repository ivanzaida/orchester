//! Controls that are on or off, beside an optional 12 `text_primary` label 8 from them; a press flips
//! them. They are form controls: Tab reaches them and Enter or Space flips them. They are lurq's `ui::Checkbox`,
//! `ui::Radio` and `ui::Switch` pressed as a whole, in the theme's recipes (`theme::kit::toggle`, which says how they
//! look): `<id>` is the whole, `<id>-label` its label, and its mark `<id>-box` (a checkbox's, its check
//! `<id>-indicator`), `<id>-mark` (a radio's, its dot `<id>-dot`) or `<id>-track` (a switch's, its thumb `<id>-thumb`).
//!
//! A control whose value is fixed is faded and cannot be flipped; the caller says why. One settled by an earlier
//! step, which a review only lists, keeps its full strength.

use std::sync::Arc;

use lurq::{
  app::theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Button, Text},
  core::ElementRef,
  layout::Alignment,
  node::{Element, dimension::Dimension},
  ui::{Checkbox, Radio, Recipe, Styled, Switch},
};

use super::pressable::{Focus, inert, pressable};
use crate::theme::{
  DISABLED_OPACITY,
  kit::toggle::{PLAIN_LABEL, RING_ROOM, SETTLED},
};

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

  /// A checkbox's label wraps onto more lines within the parent's width, the box beside its first line: a label that
  /// may be long, such as one that names a folder.
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
}

/// A checkbox's mark alone (`Checkbox/off` `q3JN1`, `Checkbox/on` `Una0j`), for a row that is pressed as a
/// whole, e.g. a filter's option: no press, no focus and hidden from tools, the row saying it is checked.
pub fn check_mark(on: bool) -> Element {
  Checkbox::new("").checked(on).mark_only(true).into()
}

/// A radio's mark alone (`Radio/off` `RchTU`, `Radio/on` `vsbEb`), for a row that is pressed as a whole, e.g. a
/// choice among grouped rows (`Y5K4K` in `Wq6tZ`).
pub fn radio_mark(on: bool) -> Element {
  Radio::new("").selected(on).mark_only(true).into()
}

/// What every control says of itself to tools, and the states the recipe draws it in.
fn styled<T: Styled>(control: T, toggle: &Toggle) -> T {
  let fixed = toggle.change.is_none();
  control
    .state(SETTLED, toggle.settled)
    .state(PLAIN_LABEL, toggle.label_style.is_none())
    .state(RING_ROOM, toggle.ring_room)
    .rules(&label_rules(toggle.label_style))
    .describe("kind", toggle.kind.name())
    .describe("checked", if toggle.on { "true" } else { "false" })
    .describe("state", if fixed { "fixed" } else { "enabled" })
}

/// The caller's style of the label, in its own color.
fn label_rules(style: Option<TypographyStyle>) -> Recipe {
  match style {
    Some(style) => Recipe::new().base("label", |part| part.typography(style)),
    None => Recipe::new(),
  }
}

impl From<Toggle> for Element {
  fn from(mut toggle: Toggle) -> Self {
    if toggle.wrap_label {
      return wrapping(toggle);
    }
    let change = toggle.change.take();
    let disabled = change.is_none();
    let wanted = move |value: bool| {
      if let Some(change) = &change {
        change(value);
      }
    };
    let element = toggle.element.take();
    let label = toggle.label.clone();
    let id = toggle.id.clone();
    let hidden = !toggle.label_shown;
    match toggle.kind {
      ToggleKind::Checkbox => {
        let mut control = Checkbox::new(&label)
          .checked(toggle.on)
          .whole(true)
          .label_hidden(hidden)
          .disabled(disabled)
          .on_change(wanted)
          .id(id);
        if let Some(element) = element {
          control = control.element_ref(element);
        }
        styled(control, &toggle).into()
      }
      ToggleKind::Radio => {
        let mut control = Radio::new(&label)
          .selected(toggle.on)
          .whole(true)
          .label_hidden(hidden)
          .disabled(disabled)
          .on_change(wanted)
          .id(id);
        if let Some(element) = element {
          control = control.element_ref(element);
        }
        styled(control, &toggle).into()
      }
      ToggleKind::Switch => {
        let mut control = Switch::new(&label)
          .on(toggle.on)
          .whole(true)
          .label_hidden(hidden)
          .disabled(disabled)
          .on_change(wanted)
          .id(id);
        if let Some(element) = element {
          control = control.element_ref(element);
        }
        styled(control, &toggle).into()
      }
    }
  }
}

/// A checkbox whose label wraps onto more lines, the box beside its first line: lurq's checkbox keeps its label on
/// one line, so the kit presses the whole and draws lurq's box alone (`<id>-box`) beside its own label.
fn wrapping(toggle: Toggle) -> Element {
  let wanted = !toggle.on;
  let button = match toggle.change {
    Some(change) => Focus::Keyboard
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
  let label = Text::new(&toggle.label).id(format!("{}-label", toggle.id));
  let label = match toggle.label_style {
    Some(style) => label.variant(style),
    None => label
      .variant(TypographyStyle::Description)
      .color(PaletteColor::Foreground),
  };
  let mark = Checkbox::new(&toggle.label)
    .checked(toggle.on)
    .mark_only(true)
    .id(format!("{}-box", toggle.id));
  button
    .id(toggle.id.to_string())
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Start)
    .spacing(SpacingSize::Sm)
    .rounded(RadiusSize::Sm)
    .describe("kind", toggle.kind.name())
    .describe("a11y_name", toggle.label.clone())
    .describe("checked", if toggle.on { "true" } else { "false" })
    .child(mark)
    .child(label.flex(1.0).min_width(Dimension::Px(0.0)))
    .into()
}
