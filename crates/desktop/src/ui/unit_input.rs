//! A single-line input with its unit inside the frame: the token budget field (`sN3du`, whole
//! tokens with a muted `tokens` unit) and the time limit field (whole minutes). An invalid value
//! gets the error stroke, 1.5px `$text`, like every other error (`T1n5BA`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::TextInputEvent,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text, TextInput},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::tokens::regroup;

use super::field::styled_input;
use crate::theme::ExtraTypography;

/// How the field is drawn: the standard field, or the compact one of New task's limits (`hWeBp`,
/// value 11, unit 10).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum UnitSize {
  Field,
  Compact,
}

pub struct UnitInput {
  id: &'static str,
  value: Signal<String>,
  unit: Arc<str>,
  invalid: bool,
  size: UnitSize,
  width: Dimension,
  height: Dimension,
  /// Groups the digits of a token count for the locale while typing.
  group_separator: Option<Arc<str>>,
  /// Lets the screen focus the input.
  focus: Option<ElementRef>,
  /// Runs when the input loses focus, e.g. to save a setting.
  blur: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl UnitInput {
  pub fn new(id: &'static str, value: Signal<String>, unit: Arc<str>) -> Self {
    Self {
      id,
      value,
      unit,
      invalid: false,
      size: UnitSize::Field,
      width: Dimension::Pct(100.0),
      height: Dimension::Pct(100.0),
      group_separator: None,
      focus: None,
      blur: None,
    }
  }

  pub fn on_blur(mut self, handler: Arc<dyn Fn() + Send + Sync>) -> Self {
    self.blur = Some(handler);
    self
  }

  /// The input is the element of `reference`, so the screen can focus it.
  pub fn focus_ref(mut self, reference: ElementRef) -> Self {
    self.focus = Some(reference);
    self
  }

  /// A token budget: digits are grouped with `separator` while they are typed.
  pub fn tokens(mut self, separator: Arc<str>) -> Self {
    self.group_separator = Some(separator);
    self
  }

  pub fn invalid(mut self, invalid: bool) -> Self {
    self.invalid = invalid;
    self
  }

  pub fn compact(mut self) -> Self {
    self.size = UnitSize::Compact;
    self
  }

  pub fn size(mut self, width: Dimension, height: Dimension) -> Self {
    self.width = width;
    self.height = height;
    self
  }

  pub fn render(self, ctx: &Ctx) -> Element {
    let input = ctx.theme().form().input.clone();
    let (value_style, unit_style) = match self.size {
      UnitSize::Field => (TypographyStyle::FieldLabel, TypographyStyle::Caption),
      UnitSize::Compact => (TypographyStyle::Caption, ExtraTypography::Hint.role()),
    };
    let border = if self.invalid {
      Border::inside(BorderSize::Md, PaletteColor::TextPrimary)
    } else {
      Border::inside(BorderSize::Sm, input.border.clone())
    };
    let mut field: TextInput = styled_input(ctx, self.value.clone(), value_style)
      .id(self.id)
      .single_line()
      .flex(1.0)
      .caret_color(PaletteColor::TextPrimary);
    if let Some(reference) = self.focus {
      field = field.ref_element(reference);
    }
    if let Some(blur) = self.blur {
      field = field.on_blur(move || blur());
    }
    if let Some(separator) = self.group_separator {
      field = field.on_input(group_while_typing(self.value.clone(), separator));
    }
    Row::new()
      .width(self.width)
      .height(self.height)
      .padding_custom(input.padding)
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .background(input.background)
      .border(border)
      .rounded(input.radius)
      .describe("state", if self.invalid { "invalid" } else { "valid" })
      .child(field)
      .child(
        Text::new(&self.unit)
          .variant(unit_style)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      )
      .into()
  }
}

/// Regroups a whole number as it is typed (`1400000` → `1,400,000`). Text that is not a whole
/// number stays as typed, so its error can name it. lurq keeps the caret at the end when it was
/// there; an edit in the middle keeps its position in the text, which a separator added before it
/// shifts by one character.
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
