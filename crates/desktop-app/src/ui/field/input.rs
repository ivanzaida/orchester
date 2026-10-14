//! The box of a text field: `surface_input` inside a `border` stroke, radius 6, 10 at its sides, the
//! value in 12 `text_primary` and the placeholder in 12 `text_muted`. A single line is 28 high (`QWfRV`)
//! with an optional 13 `text_muted` glyph before the value, 8 from it (`Field/search` `Z7mGr2`), or a
//! 12 `text_muted` unit after it (`Limit field` `eesXH`); a text area is 84 high at least, 7 over and
//! under its lines (`IBFli`). The box takes the `border_focus` stroke while its input has the focus
//! (`QWfRV` in `C0Qk5`) and the `danger` stroke while its value is not accepted (`wbLGz` in `CqBFL`).
//! A search that holds text can end in a 13 `text_muted` cross that clears it (`Clear search` `rUHLV`).
//! It types each key once: the input writes into the value itself. A locked field shows its value in
//! `text_secondary` on `surface_panel` and takes no input or focus (`Bzakf` in New task · queued
//! `H8GYVZ`).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::{TextInputEvent, mouse_event::MouseEvent},
    theme::{BorderSize, FormTextRole, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Text, TextInput},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::tokens::regroup;

use super::field_error;
use crate::{
  theme::{ControlHeight, ExtraSpacing, IconSize, SecretsText},
  ui::icon::{Icon, icon},
};

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
  width: Dimension,
  area_height: f32,
  /// A single line's box height when the design draws it taller than a control.
  line_box: Option<f32>,
  locked: bool,
  /// A 14 `text_muted` glyph at the box's end, e.g. the lock of a locked select (`H8GYVZ`).
  trailing: Option<Icon>,
  /// The value is not accepted, and something else says why: only the stroke turns `danger`.
  invalid: bool,
  masked: bool,
  on_leave: Option<Arc<dyn Fn() + Send + Sync>>,
}

/// Compared by what it shows; the value by its signal.
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
      && self.width == other.width
      && self.area_height == other.area_height
      && self.line_box == other.line_box
      && self.locked == other.locked
      && self.trailing == other.trailing
      && self.invalid == other.invalid
      && self.masked == other.masked
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
      width: Dimension::Pct(100.0),
      area_height: TEXT_AREA_HEIGHT,
      line_box: None,
      locked: false,
      trailing: None,
      invalid: false,
      masked: false,
      on_leave: None,
    }
  }

  /// Credential input: lurq masks rendering and MCP inspection and blocks copying/cutting the value.
  /// The caller must supply a signal made outside Ctx, so DevTools never records it.
  pub fn masked(mut self) -> Self {
    self.masked = true;
    self.trailing = Some(Icon::LockKeyhole);
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

  /// The value in Geist Mono, as identifiers and figures are (handover §5.2); the placeholder stays.
  /// A single line in a box `height` high, where a design draws the field taller than a control, e.g. 38 for
  /// a value 10 from each edge (`ys8f8` in Local integration).
  pub fn line_box(mut self, height: f32) -> Self {
    self.line_box = Some(height);
    self
  }

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

  pub fn render(self, ctx: &mut Ctx) -> Element {
    let key = format!("field-{}", self.id);
    ctx.mount_keyed::<FieldBox>(&key, self)
  }
}

struct FieldBox {
  focused: Signal<bool>,
}

impl Component for FieldBox {
  type Props = FieldInput;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      focused: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<FieldInput>().clone();
    let input = self.input(ctx, &props);
    let refused = props.error.is_some() || props.invalid;
    let stroke = if refused {
      PaletteColor::Danger
    } else if self.focused.get() {
      PaletteColor::BorderFocus
    } else {
      PaletteColor::Border
    };
    let dress = |frame: Row| {
      frame
        .width(props.width)
        .background(if props.locked {
          PaletteColor::SurfacePanel
        } else {
          PaletteColor::SurfaceInput
        })
        .border(Border::inside(BorderSize::Sm, stroke.clone()))
        .rounded(RadiusSize::Md)
        .describe("state", if refused { "invalid" } else { "valid" })
        .describe("focused", if self.focused.get() { "true" } else { "false" })
        .describe("locked", if props.locked { "true" } else { "false" })
    };
    let frame = match props.lines {
      Lines::One => dress(
        Row::new()
          .id(format!("{}-box", props.id))
          .height(Dimension::Px(props.line_box.unwrap_or(ControlHeight::Md.value())))
          .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center),
      )
      .with_children(
        props
          .leading
          .map(|glyph| icon(glyph, IconSize::Base, PaletteColor::TextMuted).id(format!("{}-icon", props.id))),
      )
      .child(input)
      .with_children(clear_button(&props))
      .with_children(props.unit.as_deref().map(|unit| {
        Text::new(unit)
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .nowrap()
      }))
      .with_children(props.trailing.map(|glyph| {
        icon(
          glyph,
          if props.masked { IconSize::Base } else { IconSize::Md },
          PaletteColor::TextMuted,
        )
        .id(format!("{}-trailing", props.id))
      })),
      Lines::Many | Lines::Compact => {
        dress(Row::new().id(format!("{}-box", props.id)).align_items(Alignment::Start)).child(input)
      }
    };
    let Some(error) = &props.error else {
      return Column::new().width(props.width).child(frame);
    };
    Column::new()
      .spacing(ExtraSpacing::Snug.role())
      .width(props.width)
      .child(frame)
      .child(field_error(&format!("{}-note", props.id), error))
  }
}

impl FieldBox {
  fn input(&self, ctx: &Ctx, props: &FieldInput) -> TextInput {
    let theme = ctx.theme();
    let resolve = |typography, color| FormTextRole { typography, color }.resolve(&theme.typography(), &theme.palette());
    let value = if props.masked {
      SecretsText::Credential.role()
    } else {
      props.text_style.unwrap_or(if props.mono {
        TypographyStyle::Mono
      } else {
        TypographyStyle::Description
      })
    };
    let (gained, lost) = (self.focused.clone(), self.focused.clone());
    let color = if props.locked {
      PaletteColor::TextSecondary
    } else {
      PaletteColor::TextPrimary
    };
    let mut input = TextInput::styled(props.value.clone(), resolve(value, color))
      // A mono field's placeholder reads in its own type (`The worktree` in `M20DGB`).
      .placeholder_style(resolve(value, PaletteColor::TextMuted))
      .caret_color(PaletteColor::TextPrimary)
      .id(props.id.to_string())
      .describe("a11y_name", props.name.clone())
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .on_focus(move || gained.set(true))
      .on_blur({
        let regroup = regroup_on_leave(lost, props.value.clone(), props.group_separator.clone());
        let on_leave = props.on_leave.clone();
        move || { regroup(); if let Some(commit) = &on_leave { commit(); } }
      });
    if props.masked {
      input = input.mask();
    }
    if let Some(placeholder) = &props.placeholder {
      input = input.placeholder(placeholder);
    }
    if props.locked {
      input = input
        .on_input(|event: TextInputEvent| event.prevent_default())
        .focusable(false);
    } else if let Some(separator) = props.group_separator.clone() {
      input = input.on_input(group_while_typing(props.value.clone(), separator));
    }
    match props.lines {
      Lines::One => input.single_line(),
      Lines::Many | Lines::Compact => input
        .multiline()
        .rows(1, TEXT_AREA_MAX_LINES)
        // The input owns the whole hit area, including its empty body and text insets.
        .min_height(Dimension::Px(if props.lines == Lines::Compact {
          COMPACT_AREA_HEIGHT
        } else {
          props.area_height
        }))
        .padding(Padding::symmetric(
          ExtraSpacing::Loose.role(),
          ExtraSpacing::TextArea.role(),
        )),
    }
  }
}

/// The cross that clears `props`' value, while it holds text and the field is clearable. A press clears the
/// value only: it does not also press a row around the field.
fn clear_button(props: &FieldInput) -> Option<Button> {
  let label = props.clear.clone()?;
  if props.value.get().is_empty() {
    return None;
  }
  let value = props.value.clone();
  Some(
    Button::empty()
      .id(format!("{}-clear", props.id))
      .align_items(Alignment::Center)
      .focusable(false)
      .cursor(CursorIcon::Pointer)
      .describe("a11y_name", label)
      .on_click(move |event: MouseEvent| {
        event.stop_propagation();
        value.set(String::new());
      })
      .child(icon(Icon::X, IconSize::Base, PaletteColor::TextMuted)),
  )
}

/// Leaving the field ends its focus and, for a whole number, groups its digits once more: a value set
/// without typing (pasted all at once, or by a tool) reads like a typed one.
fn regroup_on_leave(
  focused: Signal<bool>,
  value: Signal<String>,
  separator: Option<Arc<str>>,
) -> impl Fn() + Send + Sync {
  move || {
    focused.set(false);
    let Some(separator) = &separator else {
      return;
    };
    let text = value.get_untracked();
    if let Some(grouped) = regroup(&text, separator).filter(|grouped| *grouped != text) {
      value.set(grouped);
    }
  }
}

/// Regroups a whole number as it is typed (`1400000` → `1,400,000`). Text that is not a whole number
/// stays as typed, so its error can name it.
fn group_while_typing(value: Signal<String>, separator: Arc<str>) -> impl Fn(TextInputEvent) + Send + Sync + 'static {
  move |event: TextInputEvent| {
    let Some(grouped) = regroup(event.new_value(), &separator) else {
      return;
    };
    if grouped != event.new_value() {
      event.prevent_default();
      value.set(grouped);
    }
  }
}
