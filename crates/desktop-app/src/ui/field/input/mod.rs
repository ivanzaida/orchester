//! The box of a text field: `surface_input` inside a `border` stroke, radius 6, 10 at its sides, the
//! value in 12 `text_primary` and the placeholder in 12 `text_muted`. A single line is 28 high (`QWfRV`)
//! with an optional 13 `text_muted` glyph before the value, 8 from it (`Field/search` `Z7mGr2`), or a
//! 12 `text_muted` unit after it (`Limit field` `eesXH`); a text area is 84 high at least, 7 over and
//! under its lines (`IBFli`). The box takes the `border_focus` stroke while its input has the focus
//! (`QWfRV` in `C0Qk5`) and the `danger` stroke while its value is not accepted (`wbLGz` in `CqBFL`).
//! A search that holds text can end in a 13 `text_muted` cross that clears it (`Clear search` `rUHLV`);
//! an empty one can end in the key cap of the shortcut that reaches it (`Kbd` in `nsdST`).
//! It types each key once: the input writes into the value itself. A locked field shows its value in
//! `text_secondary` on `surface_panel` and takes no input or focus (`Bzakf` in New task · queued
//! `H8GYVZ`); a cut one ends in an ellipsis where it is longer than the box.

mod field_box;

use std::sync::Arc;

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx, events::TextInputEvent, theme::TypographyStyle},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};

use self::field_box::FieldBox;
use crate::{theme::ControlHeight, ui::icon::Icon};

/// A text area's least height (`IBFli` in `s72p3L`).
const TEXT_AREA_HEIGHT: f32 = 84.0;
/// Compact command fields in `QTEaI` and `A4zqOZ`.
const COMPACT_AREA_HEIGHT: f32 = 50.0;
/// The most lines a text area grows to before its lines scroll.
const TEXT_AREA_MAX_LINES: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lines {
  One,
  Many,
  Compact,
}

/// Told of an edit as it is typed ([`FieldInput::on_typed`]).
type OnTyped = Arc<dyn Fn(&TextInputEvent) + Send + Sync>;

#[derive(Clone)]
pub struct FieldInput {
  id: Arc<str>,
  value: Signal<String>,
  placeholder: Option<Arc<str>>,
  leading: Option<Icon>,
  unit: Option<Arc<str>>,
  lines: Lines,
  mono: bool,
  text_style: Option<TypographyStyle>,
  name: Arc<str>,
  /// Why the value is not accepted, said under the box, whose stroke turns `danger`.
  error: Option<Arc<str>>,
  /// Groups the digits of a whole number while it is typed.
  group_separator: Option<Arc<str>>,
  /// The cross that clears a value, named this for tools.
  clear: Option<Arc<str>>,
  /// The keys of the shortcut that reaches the field, in a cap at its end while it is empty.
  keys: Option<Arc<str>>,
  width: Dimension,
  area_height: f32,
  /// A single-line box's height.
  box_height: f32,
  /// The `border_strong` stroke at rest, where a design draws the field so.
  strong: bool,
  locked: bool,
  /// A locked value shown as a text that ends in an ellipsis where the box is narrower.
  cut: bool,
  /// A 14 `text_muted` glyph at the box's end, e.g. the lock of a locked select (`H8GYVZ`).
  trailing: Option<Icon>,
  /// The value is not accepted, and something else says why: only the stroke turns `danger`.
  invalid: bool,
  masked: bool,
  on_leave: Option<Arc<dyn Fn() + Send + Sync>>,
  /// Told of each edit as it is typed, before it lands, e.g. to tell a person's typing from automation's.
  on_typed: Option<OnTyped>,
  /// The input, for its owner to focus.
  element: Option<ElementRef>,
  /// Its place among the window's first Tab stops ([`FieldInput::tab_first`]).
  tab_first: Option<i32>,
}

/// Compared by what it shows; the value by its signal. The input's element ref is its owner's, kept for
/// the owner's life.
impl PartialEq for FieldInput {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.value.id() == other.value.id()
      && self.placeholder == other.placeholder
      && self.leading == other.leading
      && self.unit == other.unit
      && self.lines == other.lines
      && self.mono == other.mono
      && self.text_style == other.text_style
      && self.name == other.name
      && self.error == other.error
      && self.group_separator == other.group_separator
      && self.clear == other.clear
      && self.keys == other.keys
      && self.width == other.width
      && self.area_height == other.area_height
      && self.box_height == other.box_height
      && self.strong == other.strong
      && self.locked == other.locked
      && self.cut == other.cut
      && self.trailing == other.trailing
      && self.invalid == other.invalid
      && self.masked == other.masked
      && self.tab_first == other.tab_first
  }
}

impl DevtoolsInspectable for FieldInput {}

impl FieldInput {
  /// A single-line field typing into `value`, named `name` for tools (its label); its input's element
  /// id is `id`, its box's `<id>-box` and its error's `<id>-note`.
  pub fn new(id: impl Into<Arc<str>>, name: impl Into<Arc<str>>, value: Signal<String>) -> Self {
    Self {
      id: id.into(),
      value,
      placeholder: None,
      leading: None,
      unit: None,
      lines: Lines::One,
      mono: false,
      text_style: None,
      name: name.into(),
      error: None,
      group_separator: None,
      clear: None,
      keys: None,
      width: Dimension::Pct(100.0),
      area_height: TEXT_AREA_HEIGHT,
      box_height: ControlHeight::Md.value(),
      strong: false,
      locked: false,
      cut: false,
      trailing: None,
      invalid: false,
      masked: false,
      on_leave: None,
      on_typed: None,
      element: None,
      tab_first: None,
    }
  }

  /// Credential input: lurq masks rendering and MCP inspection and blocks copying/cutting the value.
  /// The caller must supply a signal made outside Ctx, so DevTools never records it.
  pub fn masked(mut self) -> Self {
    self.masked = true;
    self.trailing = Some(Icon::LockKeyhole);
    self
  }

  /// No glyph at the box's end, where the design draws a masked field without its lock (`DUWM5`'s password).
  pub fn no_trailing(mut self) -> Self {
    self.trailing = None;
    self
  }

  pub fn placeholder(mut self, placeholder: impl Into<Arc<str>>) -> Self {
    self.placeholder = Some(placeholder.into());
    self
  }

  /// A search field: the magnifying glass before the value.
  pub fn search(mut self) -> Self {
    self.leading = Some(Icon::Search);
    self
  }

  /// A glyph before the value that says what the field is for (`pencil-line` in the plan's note `c2mWh`).
  pub fn leading(mut self, glyph: Icon) -> Self {
    self.leading = Some(glyph);
    self
  }

  /// The key cap of the shortcut that reaches the field, at its end while it is empty.
  pub fn keys(mut self, keys: impl Into<Arc<str>>) -> Self {
    self.keys = Some(keys.into());
    self
  }

  /// While it holds text, the box ends in a cross that clears it; `label` names the cross for tools.
  pub fn clearable(mut self, label: impl Into<Arc<str>>) -> Self {
    self.clear = Some(label.into());
    self
  }

  /// The unit of the value, after it inside the box.
  pub fn unit(mut self, unit: impl Into<Arc<str>>) -> Self {
    self.unit = Some(unit.into());
    self
  }

  /// A text area: lines wrap and add up as they are typed.
  pub fn text_area(mut self) -> Self {
    self.lines = Lines::Many;
    self
  }

  /// A compact multiline field, as command arguments and variables are in Tools.
  pub fn compact_text_area(mut self) -> Self {
    self.lines = Lines::Compact;
    self
  }

  /// A compact inspector text area, retaining the input's full hit area and multiline growth.
  pub fn compact_area(mut self) -> Self {
    self.lines = Lines::Many;
    self.area_height = ControlHeight::Md.value();
    self
  }

  /// A text area with the design's least height, retaining growth and scrolling for longer input.
  pub fn area_height(mut self, height: f32) -> Self {
    self.area_height = height;
    self
  }

  /// A single-line box `height` high, where the design draws it taller than a control.
  pub fn box_height(mut self, height: f32) -> Self {
    self.box_height = height;
    self
  }

  /// At rest the box takes the `border_strong` stroke, e.g. a field in an inspector card (`ys8f8` in Local
  /// integration).
  pub fn strong_stroke(mut self) -> Self {
    self.strong = true;
    self
  }

  /// The value in Geist Mono, as identifiers and figures are (handover §5.2); the placeholder stays.
  pub fn mono(mut self) -> Self {
    self.mono = true;
    self
  }

  /// The design's text role for the value and placeholder.
  pub fn text_style(mut self, style: TypographyStyle) -> Self {
    self.text_style = Some(style);
    self
  }

  /// Why the value is not accepted: said under the box, whose stroke turns `danger`.
  pub fn error(mut self, error: impl Into<Arc<str>>) -> Self {
    self.error = Some(error.into());
    self
  }

  /// A whole number whose digits are grouped with `separator` while they are typed (`1,400,000`).
  pub fn grouped(mut self, separator: impl Into<Arc<str>>) -> Self {
    self.group_separator = Some(separator.into());
    self
  }

  pub fn width(mut self, width: impl Into<Dimension>) -> Self {
    self.width = width.into();
    self
  }

  /// The value cannot be changed now: it shows on `surface_panel` in `text_secondary`, and the input
  /// takes neither keys nor the focus.
  pub fn locked(mut self) -> Self {
    self.locked = true;
    self
  }

  /// Locked, and a value longer than the box ends in an ellipsis instead of running past it (a project's address in
  /// `L5o9Xy`): the value is a text then, not an input.
  pub fn cut(mut self) -> Self {
    self.locked = true;
    self.cut = true;
    self
  }

  /// A glyph at the box's end.
  pub fn trailing(mut self, glyph: Icon) -> Self {
    self.trailing = Some(glyph);
    self
  }

  /// The value is not accepted and the caller says why: the stroke turns `danger`, no note is added.
  pub fn invalid(mut self) -> Self {
    self.invalid = true;
    self
  }

  /// Commits an autosaving field after its edit and grouping have landed.
  pub fn on_leave(mut self, handler: impl Fn() + Send + Sync + 'static) -> Self {
    self.on_leave = Some(Arc::new(handler));
    self
  }

  /// `typed` is told of each edit as it is typed, before it lands in the value, and before a field that
  /// groups digits regroups it. A locked field takes no input, so it is never told.
  pub fn on_typed(mut self, typed: impl Fn(&TextInputEvent) + Send + Sync + 'static) -> Self {
    self.on_typed = Some(Arc::new(typed));
    self
  }

  /// `element` finds the input, e.g. to focus it with `Ctx::focus`.
  pub fn element(mut self, element: ElementRef) -> Self {
    self.element = Some(element);
    self
  }

  /// Tab reaches the field `position`th (from 1) of the window's first stops, before the stops in the order the window
  /// shows them, where a control drawn over the field comes after it, as *Forgot password?* over the gate's password.
  pub fn tab_first(mut self, position: i32) -> Self {
    self.tab_first = Some(position);
    self
  }

  pub fn render(self, ctx: &mut Ctx) -> Element {
    let key = format!("field-{}", self.id);
    ctx.mount_keyed::<FieldBox>(&key, self)
  }
}
